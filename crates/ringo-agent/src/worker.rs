//! The agent worker: one process, one registered UA (with `catchall`), driven
//! over stdio with the `proto` framed protocol. Because each worker is its own
//! process it binds its own SIP socket, so the provider routes incoming calls to
//! *this* registration by contact address — which in-process multi-UA cannot do
//! (one shared socket, and the request-URI user need not identify the UA, so
//! there's no way to demux).
//!
//! Channels: stdin carries the [`crate::AgentConfig`] handshake (first frame) then
//! command/query control frames + inbound audio frames; stdout carries
//! event/reply/header control frames + received-audio frames (a single writer
//! thread serialises them, so frames never interleave); stderr carries logs /
//! SIP traces.

use crate::audio;
use crate::proto::{
    self, Command, FromWorker, Headers, Query, QueryEnvelope, Ready, Reply, ReplyResult,
    RxAudioStarted, ToWorker, WireEvent, WireMediaStats,
};
use anyhow::{Context, Result, bail};
use ringo_core::backend::{Backend, BaresipBackend};
use ringo_core::phone::Phone;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// An item queued for the single stdout writer: a JSON control message or a raw
/// audio frame (mono s16 PCM, the agent's received audio).
enum Out {
    Control(FromWorker),
    Audio(Vec<i16>),
}

/// How often to forward newly-seen inbound INVITE headers to the parent.
const HEADER_POLL_INTERVAL: Duration = Duration::from_millis(150);

/// Bound on the worker's outbound (stdout) message queue. Caps memory if the
/// parent is briefly slow to drain; a full queue applies backpressure (blocking
/// send) rather than dropping events, which the parent's state machine needs.
const WRITER_CHANNEL_BOUND: usize = 1024;

/// Max wait for the de-REGISTER to be transmitted before force-stopping. The
/// registrar drops the binding on receipt (we don't need the 200 OK), and the RE
/// loop transmits the queued request within a tick, so this is a small upper
/// bound — exited early if the registration state clears first.
const UNREGISTER_TIMEOUT: Duration = Duration::from_millis(600);

/// Run the worker: read the config handshake, spawn the backend, then pump
/// stdin (commands/queries) and stdout (events/replies/headers) until stdin
/// closes (EOF) or a `Shutdown` command arrives.
pub fn run() -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();

    // Handshake: the first frame is a control frame carrying a versioned envelope
    // (protocol version + agent config).
    let handshake: proto::Handshake = match proto::read_frame(&mut stdin) {
        Ok(Some((proto::FRAME_CONTROL, payload))) => {
            serde_json::from_slice(&payload).context("parse config handshake")?
        }
        Ok(Some(_)) => bail!("worker: first frame was not the config handshake"),
        Ok(None) => bail!("worker stdin closed before config handshake"),
        Err(e) => return Err(e).context("read config handshake"),
    };
    if handshake.proto_version != proto::PROTO_VERSION {
        bail!(
            "worker: protocol version mismatch (parent {}, worker {}); rebuild so \
             both sides share one binary",
            handshake.proto_version,
            proto::PROTO_VERSION
        );
    }
    let config = handshake.config;

    // Logs / SIP traces go to the destination the parent chose (inherited via
    // env); the framed stream stays reserved for the protocol. Off by default.
    init_logging(&config.name);

    let account = config.account;
    let options = config.options;
    let username = account.username.clone();

    // The FFI backend ignores the tokio handle (it runs its own RE thread), but
    // the trait wants one — a minimal runtime kept alive for the session suffices.
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .context("build worker tokio runtime")?;
    let session = BaresipBackend
        .spawn_session(rt.handle(), &config.name, &account, &options)
        .with_context(|| format!("spawn backend for `{}`", config.name))?;

    let events = session.events;
    let phone = session.phone;
    let header_poll = session.header_poll;
    let handle = session.handle;

    // Single stdout writer: every outbound frame (control or audio) funnels
    // through here so frames never interleave. Bounded so a briefly-slow parent
    // can't grow worker memory unboundedly; a full queue blocks the sender
    // (backpressure) — control is never dropped (the parent's state machine
    // depends on it). Ends when all `tx` clones drop.
    let (tx, rx) = std::sync::mpsc::sync_channel::<Out>(WRITER_CHANNEL_BOUND);
    let writer = std::thread::spawn(move || {
        let stdout = std::io::stdout();
        while let Ok(msg) = rx.recv() {
            let mut out = stdout.lock();
            let res = match msg {
                Out::Control(c) => match serde_json::to_vec(&c) {
                    Ok(bytes) => proto::write_frame(&mut out, proto::FRAME_CONTROL, &bytes),
                    Err(e) => {
                        ringo_core::rlog!(Warn, "serialize outbound: {e}");
                        continue;
                    }
                },
                Out::Audio(samples) => {
                    proto::write_frame(&mut out, proto::FRAME_AUDIO, &proto::pcm_to_bytes(&samples))
                }
            };
            if res.is_err() {
                break;
            }
        }
    });

    // Readiness handshake: the backend is up — tell the parent BEFORE anything
    // else, so `ProcessClient::spawn` returns success only once we're live.
    let _ = tx.send(Out::Control(FromWorker::Ready(Ready { ready: true })));

    // Event bridge: backend events → stdout. Ends when the backend drops its
    // event sender (the session handle is dropped on teardown).
    let ev_tx = tx.clone();
    let event_bridge = std::thread::spawn(move || {
        while let Ok(event) = events.recv() {
            if ev_tx
                .send(Out::Control(FromWorker::Event(WireEvent::from(&event))))
                .is_err()
            {
                break;
            }
        }
    });

    // Header poll: forward newly-seen inbound INVITE headers. Stops on `stop`.
    let stop = Arc::new(AtomicBool::new(false));
    let header_thread = header_poll.map(|poll| {
        let hdr_tx = tx.clone();
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(HEADER_POLL_INTERVAL);
                // poll() drains the store and returns only newly-seen INVITE
                // headers, so forward whatever it yields (the parent dedups by
                // Call-ID). A length/growth guard here would silently drop the
                // 2nd+ inbound call's headers.
                if let Some(headers) = poll() {
                    if !headers.is_empty()
                        && hdr_tx
                            .send(Out::Control(FromWorker::Headers(Headers { headers })))
                            .is_err()
                    {
                        break;
                    }
                }
            }
        })
    });

    // RX-audio forwarder: started on the first `StartRxAudio`.
    let rx_stop = Arc::new(AtomicBool::new(false));
    let mut rx_forwarder: Option<JoinHandle<()>> = None;

    // Command/query/audio loop on the remaining stdin frames.
    loop {
        let frame = match proto::read_frame(&mut stdin) {
            Ok(Some(f)) => f,
            Ok(None) => break, // clean EOF
            Err(e) => {
                ringo_core::rlog!(Warn, "worker stdin read error: {e}");
                break;
            }
        };
        match frame {
            // Inbound audio: play streamed PCM into the call (TTS).
            (proto::FRAME_AUDIO, payload) => {
                ringo_core::push_audio(&username, &proto::bytes_to_pcm(&payload));
            }
            (proto::FRAME_CONTROL, payload) => match serde_json::from_slice::<ToWorker>(&payload) {
                Ok(ToWorker::Cmd(Command::Shutdown)) => break,
                Ok(ToWorker::Cmd(Command::StartTxAudio { rate })) => {
                    ringo_core::start_audio_stream(&username, rate);
                }
                Ok(ToWorker::Cmd(Command::StartRxAudio)) => {
                    if rx_forwarder.is_none() {
                        rx_forwarder = Some(spawn_rx_forwarder(
                            &username,
                            tx.clone(),
                            Arc::clone(&rx_stop),
                        ));
                    }
                }
                Ok(ToWorker::Cmd(cmd)) => dispatch(phone.as_ref(), cmd),
                Ok(ToWorker::Query(QueryEnvelope { id, query })) => {
                    let result = answer(phone.as_ref(), &username, query);
                    if tx
                        .send(Out::Control(FromWorker::Reply(Reply { reply: id, result })))
                        .is_err()
                    {
                        break;
                    }
                }
                // Redact the payload (it may carry credentials/PII) — log only
                // the parse error and the length.
                Err(e) => ringo_core::rlog!(
                    Warn,
                    "ignoring malformed control frame ({} bytes): {e}",
                    payload.len()
                ),
            },
            (_kind, _) => {} // unknown frame kind: ignore
        }
    }

    // Teardown: stop the poll/forwarder threads, drop the UA (which also drops
    // the event sender → ends the bridge), stop the RE thread, then join writer.
    stop.store(true, Ordering::Relaxed);
    rx_stop.store(true, Ordering::Relaxed);
    if let Some(h) = header_thread {
        let _ = h.join();
    }
    if let Some(h) = rx_forwarder {
        let _ = h.join();
    }
    drop(phone);
    drop(handle); // schedules ua_unregister (de-REGISTER, expires=0) on the RE thread
    // Let the RE loop actually transmit the de-REGISTER and process its 200 OK
    // before we stop it — otherwise `shutdown` force-stops the loop and the
    // registration is left stale on the registrar (one binding leaks per run).
    let deadline = Instant::now() + UNREGISTER_TIMEOUT;
    while ringo_core::is_registered() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if ringo_core::is_registered() {
        ringo_core::rlog!(
            Warn,
            "de-REGISTER not confirmed within {}ms; registrar binding may linger",
            UNREGISTER_TIMEOUT.as_millis()
        );
    }
    ringo_core::shutdown();
    let _ = event_bridge.join();
    drop(tx);
    let _ = writer.join();
    Ok(())
}

/// Forward the agent's received audio to the parent: announce the sample rate
/// (`RxAudioStarted`) before the first frame AND again whenever it changes (a
/// codec renegotiation can switch e.g. 8k↔16k mid-call), then send each decoded
/// frame as a raw audio frame. Raw frames carry no rate, so the parent tags them
/// with the last announced rate — re-announcing keeps that tag correct. Stops on
/// `stop` (or when the subscription/writer goes away).
fn spawn_rx_forwarder(
    username: &str,
    tx: SyncSender<Out>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    let rx = ringo_core::subscribe_received_audio(username);
    std::thread::spawn(move || {
        let mut announced_rate: Option<u32> = None;
        while !stop.load(Ordering::Relaxed) {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) => {
                    if announced_rate != Some(frame.rate) {
                        announced_rate = Some(frame.rate);
                        let started = FromWorker::RxAudioStarted(RxAudioStarted {
                            rx_audio_rate: frame.rate,
                        });
                        if tx.send(Out::Control(started)).is_err() {
                            break;
                        }
                    }
                    if tx.send(Out::Audio(frame.samples)).is_err() {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => continue, // re-check stop
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    })
}

/// Configure this worker's log + SIP-trace sinks from the parent's choice,
/// inherited via env (`RINGO_AGENT_LOG` / `RINGO_AGENT_SIPTRACE`): unset = off,
/// `-` = stderr, anything else = a file. File targets are made per-agent
/// (`run.log` -> `run.<name>.log`) so concurrent workers don't share a file.
fn init_logging(name: &str) {
    if let Some(t) = std::env::var_os("RINGO_AGENT_LOG").and_then(|v| v.into_string().ok()) {
        match t.as_str() {
            "" => {}
            "-" => ringo_core::log::init_stderr(),
            path => ringo_core::log::init_file(per_agent_path(path, name)),
        }
    }
    if let Some(t) = std::env::var_os("RINGO_AGENT_SIPTRACE").and_then(|v| v.into_string().ok()) {
        match t.as_str() {
            "" => {}
            "-" => ringo_core::sip_trace_stderr(),
            path => ringo_core::sip_trace_file(per_agent_path(path, name)),
        }
    }
}

/// Insert a per-agent tag before the file extension: `run.log` ->
/// `run.<name>.<pid>.log`. The name is sanitised (no path separators / dots, so
/// it can't escape the directory) and the PID disambiguates same-named agents
/// across scenarios so their truncating logs don't clobber each other.
fn per_agent_path(base: &str, name: &str) -> std::path::PathBuf {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let tag = format!("{safe}.{}", std::process::id());
    let p = std::path::Path::new(base);
    match p.extension().and_then(|e| e.to_str()) {
        Some(ext) => p.with_extension(format!("{tag}.{ext}")),
        None => p.with_extension(tag),
    }
}

/// Apply one fire-and-forget command to the phone.
fn dispatch(phone: &dyn Phone, cmd: Command) {
    match cmd {
        Command::Register { aor, regint } => phone.register(&aor, regint),
        Command::Dial { number } => phone.dial(&number),
        Command::Hangup => phone.hangup(),
        Command::HangupAll => phone.hangup_all(),
        Command::Accept => phone.accept(),
        Command::Hold => phone.hold(),
        Command::Resume => phone.resume(),
        Command::Mute => phone.mute(),
        Command::Dtmf { digit } => phone.send_dtmf(digit),
        Command::SwitchLine { line } => phone.switch_line(line),
        Command::Transfer { uri } => phone.transfer(&uri),
        Command::AttendedTransferStart { uri } => phone.attended_transfer_start(&uri),
        Command::AttendedTransferExec => phone.attended_transfer_exec(),
        Command::AttendedTransferAbort => phone.attended_transfer_abort(),
        Command::AddHeader { key, value } => phone.add_header(&key, &value),
        Command::RmHeader { key } => phone.rm_header(&key),
        Command::SetAudioSource { spec } => phone.set_audio_source(&spec),
        Command::ArmInviteResponse {
            scode,
            reason,
            headers,
        } => phone.arm_invite_response(scode, &reason, headers),
        Command::DisarmInviteResponse => phone.disarm_invite_response(),
        // Audio-stream control + shutdown are handled in the run loop, before
        // dispatch (they need the username / writer, not just the phone).
        Command::StartTxAudio { .. } | Command::StartRxAudio | Command::Shutdown => {}
    }
}

/// Answer one query. WAV writing runs here (on the worker's own in-process
/// captured buffer) so only the resulting paths cross the pipe; tone analysis
/// now happens on the parent over the streamed RX audio.
fn answer(phone: &dyn Phone, username: &str, query: Query) -> ReplyResult {
    match query {
        Query::MediaStats => ReplyResult::MediaStats(phone.media_stats().map(WireMediaStats::from)),
        Query::ReceivedDtmf => ReplyResult::Dtmf(phone.received_dtmf()),
        Query::SaveAudio { prefix } => {
            let mut written = Vec::new();
            for (tag, captured) in [
                ("sent", ringo_core::sent_audio(username)),
                ("recv", ringo_core::received_audio(username)),
            ] {
                let Some((samples, srate)) = captured else {
                    continue;
                };
                if samples.is_empty() {
                    continue;
                }
                let path = format!("{prefix}-{tag}.wav");
                match audio::write_wav(std::path::Path::new(&path), &samples, srate) {
                    Ok(()) => written.push(path),
                    Err(e) => ringo_core::rlog!(Warn, "save {path}: {e}"),
                }
            }
            ReplyResult::Saved(written)
        }
        Query::CallCount => ReplyResult::CallCount(ringo_core::call_count() as u32),
    }
}
