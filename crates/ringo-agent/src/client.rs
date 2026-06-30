//! Parent side: [`ProcessClient`] spawns an agent worker (re-execing the host
//! binary's `agent` subcommand), performs the [`AgentConfig`] handshake, and
//! exposes the agent's full API over the [`crate::proto`] framed protocol —
//! fire-and-forget commands, id-correlated queries (blocking on the matching
//! reply), an [`AppEvent`] stream, a buffer of pushed inbound INVITE headers,
//! and live audio in/out (TTS into the call / received audio for STT).
//!
//! The consumer drives every agent through this, so there is no in-process
//! baresip UA in the parent. WAV writing happens in the worker (only the paths
//! cross the pipe); received audio for tone analysis is streamed to the parent.

use crate::proto::{
    self, AgentConfig, Command, FromWorker, Query, QueryEnvelope, ReplyResult, ToWorker,
};
use anyhow::{Context, Result};
use ringo_core::AudioFrame;
use ringo_core::event::{AppEvent, InviteHeaders, MediaStats};
use std::collections::HashMap;
use std::io::BufReader;
use std::process::{Child, ChildStdin, Command as OsCommand, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long to wait for a query reply before giving up. Replies are computed
/// synchronously in the worker (no media settle-time happens here), so this only
/// guards against a dead/stuck worker.
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// How long `spawn` waits for the worker's readiness ACK before giving up — a
/// generous bound (backend init is sub-second) so a worker stuck before `Ready`
/// fails the spawn instead of hanging it forever.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// How long to let a worker exit gracefully on teardown before killing it.
/// Must exceed the worker's de-REGISTER wait (see `UNREGISTER_TIMEOUT`) plus its
/// RE-thread shutdown, or we'd kill it mid-deregister and leak the binding.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Handle to one agent worker process. Spawn it with [`ProcessClient::spawn`],
/// then drive it: fire-and-forget call-control commands, blocking queries (media
/// stats, received DTMF, tone analysis, …), and an [`AppEvent`] stream (returned
/// by `spawn`). Dropping it deregisters and reaps the worker.
pub struct ProcessClient {
    name: String,
    stdin: Mutex<ChildStdin>,
    pending: Arc<Mutex<HashMap<u64, Sender<ReplyResult>>>>,
    next_id: AtomicU64,
    headers: Arc<Mutex<Option<InviteHeaders>>>,
    child: Mutex<Option<Child>>,
    shutdown_sent: AtomicBool,
    /// Set by the reader thread when the worker's stdout closes (exit/crash), so
    /// queries short-circuit instead of waiting out `QUERY_TIMEOUT`.
    dead: Arc<AtomicBool>,
    /// Consumer's sink for received audio frames, installed by `start_rx_audio`.
    /// (The RX sample rate lives in the reader thread, which tags each frame.)
    rx_audio: Arc<Mutex<Option<Sender<AudioFrame>>>>,
}

impl ProcessClient {
    /// Spawn a worker for `config`, returning the client and the agent's event
    /// stream (already converted to backend-neutral [`AppEvent`]s).
    pub fn spawn(config: AgentConfig) -> Result<(Self, Receiver<AppEvent>)> {
        let exe = std::env::current_exe().context("locate own executable")?;
        let mut os = OsCommand::new(exe);
        os.arg("agent")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        // Linux: ask the kernel to SIGTERM the worker if the parent dies, so a
        // killed/crashed/Ctrl-C'd parent can't orphan a still-registered worker.
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            // SAFETY: the closure runs in the child after fork, before exec, and
            // calls only async-signal-safe libc functions.
            unsafe {
                os.pre_exec(|| {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    // Race: the parent may have already exited before prctl ran.
                    if libc::getppid() == 1 {
                        libc::raise(libc::SIGTERM);
                    }
                    Ok(())
                });
            }
        }
        let mut child = os
            .spawn()
            .with_context(|| format!("spawn agent worker for `{}`", config.name))?;

        let mut stdin = child.stdin.take().context("worker stdin missing")?;
        let stdout = child.stdout.take().context("worker stdout missing")?;

        // Handshake: a versioned envelope carrying the config is the first
        // control frame on stdin (keeps credentials out of argv/environ).
        let handshake = proto::Handshake {
            proto_version: proto::PROTO_VERSION,
            config,
        };
        let cfg_bytes = serde_json::to_vec(&handshake).context("serialize handshake")?;
        proto::write_frame(&mut stdin, proto::FRAME_CONTROL, &cfg_bytes)
            .context("write config handshake")?;

        let (ev_tx, ev_rx) = channel::<AppEvent>();
        let pending: Arc<Mutex<HashMap<u64, Sender<ReplyResult>>>> = Arc::default();
        let headers: Arc<Mutex<Option<InviteHeaders>>> = Arc::default();
        let dead = Arc::new(AtomicBool::new(false));
        let rx_audio: Arc<Mutex<Option<Sender<AudioFrame>>>> = Arc::default();
        let rx_audio_rate = Arc::new(AtomicU32::new(0));
        // The reader signals readiness here once the worker emits its `Ready`
        // ACK; spawn() waits on it with a bound, so a worker stuck before `Ready`
        // (or one that exited → reader EOF → `ready_tx` dropped) fails the spawn
        // instead of hanging it forever.
        let (ready_tx, ready_rx) = channel::<()>();

        // Reader: demux worker stdout frames — control (readiness/events/replies/
        // header pushes/rx-audio start) and raw received-audio frames.
        let r_pending = Arc::clone(&pending);
        let r_headers = Arc::clone(&headers);
        let r_dead = Arc::clone(&dead);
        let r_rx_audio = Arc::clone(&rx_audio);
        let r_rx_rate = Arc::clone(&rx_audio_rate);
        let label = handshake.config.name.clone();
        let mut reader = BufReader::new(stdout);
        std::thread::spawn(move || {
            loop {
                let (kind, payload) = match proto::read_frame(&mut reader) {
                    Ok(Some(f)) => f,
                    Ok(None) => break, // clean EOF
                    Err(e) => {
                        ringo_core::rlog!(Warn, "agent `{label}`: read error: {e}");
                        break;
                    }
                };
                if kind == proto::FRAME_AUDIO {
                    // Received audio → consumer sink, tagged with the announced rate.
                    let sink = r_rx_audio.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(tx) = sink.as_ref() {
                        let frame = AudioFrame {
                            samples: proto::bytes_to_pcm(&payload),
                            rate: r_rx_rate.load(Ordering::Relaxed),
                        };
                        let _ = tx.send(frame);
                    }
                    continue;
                }
                if kind != proto::FRAME_CONTROL {
                    // Unknown frame kind: ignore (forward-compatible), matching the
                    // worker's stdin loop. The version handshake makes this unlikely.
                    continue;
                }
                match serde_json::from_slice::<FromWorker>(&payload) {
                    Ok(FromWorker::Ready(_)) => {
                        let _ = ready_tx.send(());
                    }
                    Ok(FromWorker::Event(w)) => {
                        if ev_tx.send(w.into()).is_err() {
                            break; // parent dropped the receiver
                        }
                    }
                    Ok(FromWorker::Reply(r)) => {
                        if let Some(tx) = r_pending
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(&r.reply)
                        {
                            let _ = tx.send(r.result);
                        }
                    }
                    Ok(FromWorker::Headers(h)) => {
                        // Merge, don't overwrite: two pushes between the parent's
                        // poll/take must both survive (the parent dedups by
                        // Call-ID), or the earlier one's headers are lost.
                        let mut buf = r_headers.lock().unwrap_or_else(|e| e.into_inner());
                        match buf.as_mut() {
                            Some(existing) => existing.extend(h.headers),
                            None => *buf = Some(h.headers),
                        }
                    }
                    Ok(FromWorker::RxAudioStarted(s)) => {
                        r_rx_rate.store(s.rx_audio_rate, Ordering::Relaxed);
                    }
                    // Redact the payload (it may carry PII): log only the length.
                    Err(e) => ringo_core::rlog!(
                        Warn,
                        "agent `{label}`: bad control frame ({} bytes): {e}",
                        payload.len()
                    ),
                }
            }
            // Worker stdout closed (exit/crash): mark dead and release every
            // blocked query so callers return immediately instead of waiting out
            // QUERY_TIMEOUT. Set `dead` UNDER the pending lock so it's ordered
            // against `query`'s under-lock re-check (closes the insert-after-clear
            // race); the mutex provides the happens-before for the Relaxed store.
            let mut pending = r_pending.lock().unwrap_or_else(|e| e.into_inner());
            r_dead.store(true, Ordering::Relaxed);
            pending.clear();
        });

        // Block (bounded) on the readiness ACK before handing back the client.
        match ready_rx.recv_timeout(READY_TIMEOUT) {
            Ok(()) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!(
                    "agent `{}` not ready within {:?} (bad config / failed backend / exited)",
                    handshake.config.name,
                    READY_TIMEOUT
                );
            }
        }

        Ok((
            Self {
                name: handshake.config.name,
                stdin: Mutex::new(stdin),
                pending,
                next_id: AtomicU64::new(0),
                headers,
                child: Mutex::new(Some(child)),
                shutdown_sent: AtomicBool::new(false),
                dead,
                rx_audio,
            },
            ev_rx,
        ))
    }

    /// A clone of the inbound-header buffer, for building a `header_poll` closure.
    pub fn headers_handle(&self) -> Arc<Mutex<Option<InviteHeaders>>> {
        Arc::clone(&self.headers)
    }

    fn send(&self, msg: ToWorker) {
        let bytes = match serde_json::to_vec(&msg) {
            Ok(b) => b,
            Err(e) => {
                ringo_core::rlog!(Warn, "agent `{}`: serialize message: {e}", self.name);
                return;
            }
        };
        let mut w = self.stdin.lock().unwrap_or_else(|e| e.into_inner());
        if proto::write_frame(&mut *w, proto::FRAME_CONTROL, &bytes).is_err() {
            ringo_core::rlog!(Warn, "agent `{}`: worker stdin closed", self.name);
        }
    }

    fn cmd(&self, cmd: Command) {
        self.send(ToWorker::Cmd(cmd));
    }

    /// Tell the worker to shut down (deregister + exit) without blocking. Sent
    /// at most once; call this on all agents before dropping them so their
    /// de-REGISTERs run concurrently instead of one-at-a-time on each drop.
    pub fn request_shutdown(&self) {
        if !self.shutdown_sent.swap(true, Ordering::Relaxed) {
            self.cmd(Command::Shutdown);
        }
    }

    /// Send a query and block until the matching reply (or timeout/dead worker).
    fn query(&self, query: Query) -> Option<ReplyResult> {
        // Don't even try if the worker is gone — its reply will never come.
        if self.dead.load(Ordering::Relaxed) {
            return None;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = channel::<ReplyResult>();
        {
            // Register the waiter and re-check `dead` under the same lock the
            // reader takes to drain on EOF. Without this, the worker could die
            // between the fast-path check above and the insert, orphaning the
            // entry and stalling the full QUERY_TIMEOUT.
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            if self.dead.load(Ordering::Relaxed) {
                return None;
            }
            pending.insert(id, tx);
        }
        self.send(ToWorker::Query(QueryEnvelope { id, query }));
        match rx.recv_timeout(QUERY_TIMEOUT) {
            Ok(r) => Some(r),
            Err(_) => {
                self.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                ringo_core::rlog!(Warn, "agent `{}`: query timed out", self.name);
                None
            }
        }
    }

    // ── commands (fire-and-forget) ──────────────────────────────────────────
    /// Register the account at `aor` with the given re-register interval (s).
    pub fn register(&self, aor: &str, regint: u32) {
        self.cmd(Command::Register {
            aor: aor.to_string(),
            regint,
        });
    }
    /// Place an outgoing call to `number` (a SIP URI or a bare number/extension).
    pub fn dial(&self, number: &str) {
        self.cmd(Command::Dial {
            number: number.to_string(),
        });
    }
    /// Hang up the active call.
    pub fn hangup(&self) {
        self.cmd(Command::Hangup);
    }
    /// Hang up all calls.
    pub fn hangup_all(&self) {
        self.cmd(Command::HangupAll);
    }
    /// Answer the incoming call.
    pub fn accept(&self) {
        self.cmd(Command::Accept);
    }
    /// Put the active call on hold.
    pub fn hold(&self) {
        self.cmd(Command::Hold);
    }
    /// Resume the held call.
    pub fn resume(&self) {
        self.cmd(Command::Resume);
    }
    /// Mute the active call's outgoing audio.
    pub fn mute(&self) {
        self.cmd(Command::Mute);
    }
    /// Send a single DTMF digit on the active call.
    pub fn send_dtmf(&self, digit: char) {
        self.cmd(Command::Dtmf { digit });
    }
    /// Not yet exposed by a scenario verb (line switching is planned).
    #[allow(dead_code)]
    pub fn switch_line(&self, line: usize) {
        self.cmd(Command::SwitchLine { line });
    }
    /// Blind-transfer the active call to `uri`.
    pub fn transfer(&self, uri: &str) {
        self.cmd(Command::Transfer {
            uri: uri.to_string(),
        });
    }
    /// Start an attended transfer: call `uri` as a consultation call.
    pub fn attended_transfer_start(&self, uri: &str) {
        self.cmd(Command::AttendedTransferStart {
            uri: uri.to_string(),
        });
    }
    /// Complete the attended transfer (connect the two parties).
    pub fn attended_transfer_exec(&self) {
        self.cmd(Command::AttendedTransferExec);
    }
    /// Abort the attended transfer (hang up the consultation call).
    pub fn attended_transfer_abort(&self) {
        self.cmd(Command::AttendedTransferAbort);
    }
    /// Add a custom SIP header sent on subsequent requests.
    pub fn add_header(&self, key: &str, value: &str) {
        self.cmd(Command::AddHeader {
            key: key.to_string(),
            value: value.to_string(),
        });
    }
    /// Not yet exposed by a scenario verb (header removal is planned).
    #[allow(dead_code)]
    pub fn rm_header(&self, key: &str) {
        self.cmd(Command::RmHeader {
            key: key.to_string(),
        });
    }
    /// Switch the active call's audio source (e.g. a tone, a file, or silence).
    pub fn set_audio_source(&self, spec: &str) {
        self.cmd(Command::SetAudioSource {
            spec: spec.to_string(),
        });
    }
    /// Arm a fixed response (status `scode`/`reason` + extra `headers`) for the
    /// next inbound INVITE instead of accepting it.
    pub fn arm_invite_response(&self, scode: u16, reason: &str, headers: Vec<String>) {
        self.cmd(Command::ArmInviteResponse {
            scode,
            reason: reason.to_string(),
            headers,
        });
    }
    /// Clear a previously armed invite response (accept incoming calls again).
    pub fn disarm_invite_response(&self) {
        self.cmd(Command::DisarmInviteResponse);
    }
    /// Mirror of the `Phone::deflect_incoming` default: arm a 302 with `Contact`
    /// (and optional RFC 5806 `Diversion`).
    pub fn deflect_incoming(&self, contact: &str, diversion: Option<&str>) {
        let mut headers = vec![format!("Contact: <{contact}>")];
        if let Some(div) = diversion {
            headers.push(format!("Diversion: <{div}>"));
        }
        self.arm_invite_response(302, "Moved Temporarily", headers);
    }

    // ── queries (block on the worker's reply) ────────────────────────────────
    /// RTP media stats for the active/last call, or `None` if unavailable.
    pub fn media_stats(&self) -> Option<MediaStats> {
        match self.query(Query::MediaStats) {
            Some(ReplyResult::MediaStats(s)) => s.map(Into::into),
            _ => None,
        }
    }
    /// DTMF digits received on the active/last call so far, in order.
    pub fn received_dtmf(&self) -> String {
        match self.query(Query::ReceivedDtmf) {
            Some(ReplyResult::Dtmf(s)) => s,
            _ => String::new(),
        }
    }
    /// Ask the worker to write its captured audio to `<prefix>-sent.wav` /
    /// `<prefix>-recv.wav`; returns the paths written.
    pub fn save_audio(&self, prefix: &str) -> Vec<String> {
        match self.query(Query::SaveAudio {
            prefix: prefix.to_string(),
        }) {
            Some(ReplyResult::Saved(paths)) => paths,
            _ => Vec::new(),
        }
    }
    /// Number of active calls in the worker (0 if the worker is gone).
    pub fn call_count(&self) -> u32 {
        match self.query(Query::CallCount) {
            Some(ReplyResult::CallCount(n)) => n,
            _ => 0,
        }
    }

    // ── live audio ───────────────────────────────────────────────────────────
    // TX (audio into the call) has no in-tree consumer yet: it's the building
    // block for a live producer such as a TTS-driven MCP tool (`ringo-mcp`). RX
    // (audio out of the call) is consumed by ringo-flow's tone verification.
    /// Switch the agent's audio source to live-streamed mono s16 PCM at `rate` Hz;
    /// feed it with [`Self::push_tx_audio`] (e.g. TTS output). Call this and feed
    /// `push_tx_audio` from the *same* thread: the worker requires the
    /// `StartTxAudio` control frame to arrive before any audio frame (it drops
    /// audio that arrives first), and the stdin lock is not FIFO across threads.
    pub fn start_tx_audio(&self, rate: u32) {
        self.cmd(Command::StartTxAudio { rate });
    }

    /// Stream mono s16 PCM into the call as a raw audio frame (after
    /// [`Self::start_tx_audio`], from the same thread).
    pub fn push_tx_audio(&self, samples: &[i16]) {
        let mut w = self.stdin.lock().unwrap_or_else(|e| e.into_inner());
        if proto::write_frame(&mut *w, proto::FRAME_AUDIO, &proto::pcm_to_bytes(samples)).is_err() {
            ringo_core::rlog!(
                Warn,
                "agent `{}`: worker stdin closed (tx audio)",
                self.name
            );
        }
    }

    /// Start streaming the agent's received audio; returns a receiver of mono
    /// [`AudioFrame`]s (e.g. to feed STT or a parent-side tone analysis). One
    /// subscription per client.
    pub fn start_rx_audio(&self) -> Receiver<AudioFrame> {
        let (tx, rx) = channel();
        *self.rx_audio.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
        self.cmd(Command::StartRxAudio);
        rx
    }
}

impl Drop for ProcessClient {
    fn drop(&mut self) {
        // Ask the worker to exit, then reap it (kill only if it overstays).
        // Idempotent: a no-op if teardown already requested shutdown.
        self.request_shutdown();
        if let Some(mut child) = self.child.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let deadline = Instant::now() + SHUTDOWN_GRACE;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => return, // exited cleanly
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    _ => break, // overstayed its grace, or wait() errored
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
