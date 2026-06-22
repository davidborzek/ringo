//! A live agent session: one headless baresip instance plus its ctrl_tcp
//! connection. Incoming events are folded into an [`AgentState`] (published over
//! a `watch` channel) that the runner asserts against.

use super::state::{AgentState, reduce};
use anyhow::{Context, Result};
use ringo_core::baresip::{Account, BaresipOptions, Instance};
use ringo_core::client;
use ringo_core::event::AppEvent;
use ringo_core::phone::{BaresipPhone, Phone};
use ringo_core::siptrace;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};

/// How often the reader task re-scans the SIP-trace log for inbound headers.
const TRACE_POLL_INTERVAL: Duration = Duration::from_millis(150);

pub struct AgentSession {
    pub aor: String,
    pub regint: u32,
    phone: BaresipPhone,
    state_rx: watch::Receiver<AgentState>,
    // Kept alive for the session's lifetime; dropping kills baresip + cleans up.
    _instance: Instance,
}

impl AgentSession {
    /// Spawn baresip for an account, connect to its ctrl_tcp port and fold
    /// events into shared state. `name` labels the instance/logs.
    pub async fn connect(name: &str, account: Account, options: &BaresipOptions) -> Result<Self> {
        let instance = Instance::spawn(name, &account, options)
            .with_context(|| format!("spawn baresip for `{name}`"))?;

        let stream = connect_retry(instance.port, Duration::from_secs(10))
            .await
            .with_context(|| format!("connect to baresip ctrl_tcp for `{name}`"))?;
        let (mut reader, mut writer) = stream.into_split();

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<(String, String)>(32);
        let (state_tx, state_rx) = watch::channel(AgentState::default());

        // ctrl_tcp events and the SIP-trace poll both update the same state, but
        // run on SEPARATE tasks. They must NOT share one `select!`: that would
        // cancel an in-flight `client::read_message` (which is NOT
        // cancellation-safe — it reads a netstring across several `read_exact`s)
        // every time the poll timer fired, dropping partially-read bytes and
        // desyncing the ctrl_tcp stream (lost REGISTER_OK / CALL_INCOMING …).
        // The watch sender is shared via `Arc` (its methods take `&self`).
        let state_tx = Arc::new(state_tx);

        // Reader: ctrl_tcp events → state. Cancellation-free (no select!).
        let reader_tx = Arc::clone(&state_tx);
        tokio::spawn(async move {
            while let Ok(msg) = client::read_message(&mut reader).await {
                let event = AppEvent::from(msg);
                reader_tx.send_modify(|s| reduce(s, &event));
            }
        });

        // Trace poll: inbound INVITE headers → state (the events don't carry
        // them). Stops once the session's receivers are gone.
        let log_path = instance.log_path.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(TRACE_POLL_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut tail = siptrace::TraceTail::new();
            loop {
                ticker.tick().await;
                if state_tx.is_closed() {
                    break; // session dropped
                }
                // Run the file read + whole-file parse on the blocking pool, not
                // on an async worker: both are blocking/CPU work (the parse grows
                // with the log) and must not stall ctrl_tcp I/O — e.g. while an
                // `http` step is also using the runtime, which otherwise delayed
                // commands like `dial`. `tail` moves in and back out.
                let lp = log_path.clone();
                let (returned, invites) =
                    match tokio::task::spawn_blocking(move || (tail.poll(&lp), tail)).await {
                        Ok((invites, t)) => (t, invites),
                        Err(_) => break, // blocking task failed
                    };
                tail = returned;
                if let Some(invites) = invites {
                    merge_received_headers(&state_tx, invites);
                }
            }
        });

        // Writer: queued commands → baresip.
        tokio::spawn(async move {
            while let Some((cmd, params)) = cmd_rx.recv().await {
                if client::write_command(&mut writer, &cmd, &params)
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        Ok(Self {
            aor: format!("sip:{}@{}", account.username, account.domain),
            regint: account.regint.unwrap_or(3600),
            phone: BaresipPhone::new(cmd_tx),
            state_rx,
            _instance: instance,
        })
    }

    /// This agent's SIP domain (the host part of its AOR), used to build a full
    /// request URI when dialing a bare number/extension.
    pub fn domain(&self) -> &str {
        self.aor.rsplit('@').next().unwrap_or("")
    }

    /// A fresh receiver on this agent's state (used by `wait` to guard calls).
    pub fn state(&self) -> watch::Receiver<AgentState> {
        self.state_rx.clone()
    }

    /// Path to this agent's baresip log (valid until the session is dropped).
    pub fn log_path(&self) -> &Path {
        &self._instance.log_path
    }

    /// Directory holding this agent's recordings (baresip's working dir == the
    /// log's parent; sndfile writes `dump-…-dec.wav` here).
    pub fn recording_dir(&self) -> &Path {
        self._instance.log_path.parent().unwrap_or(Path::new("."))
    }

    /// Switch the agent's audio source on its active call (baresip `ausrc`).
    pub fn set_audio_source(&self, spec: &str) {
        self.phone.set_audio_source(spec);
    }

    // ── Commands ────────────────────────────────────────────────────────────

    pub fn register(&self) {
        self.phone.register(&self.aor, self.regint);
    }
    pub fn dial(&self, target: &str) {
        self.phone.dial(target);
    }
    pub fn accept(&self) {
        self.phone.accept();
    }
    pub fn hold(&self) {
        self.phone.hold();
    }
    pub fn resume(&self) {
        self.phone.resume();
    }
    pub fn mute(&self) {
        self.phone.mute();
    }
    pub fn send_dtmf(&self, digit: char) {
        self.phone.send_dtmf(digit);
    }
    /// Add a custom SIP header to the agent's outgoing requests (UA-level).
    pub fn add_header(&self, key: &str, value: &str) {
        self.phone.add_header(key, value);
    }
    pub fn hangup(&self) {
        self.phone.hangup();
    }
    pub fn hangup_all(&self) {
        self.phone.hangup_all();
    }
    /// Blind transfer (REFER) of the active call to `uri`.
    pub fn transfer(&self, uri: &str) {
        self.phone.transfer(uri);
    }
    /// Start an attended transfer: place a consultation call to `uri`.
    pub fn attended_transfer_start(&self, uri: &str) {
        self.phone.attended_transfer_start(uri);
    }
    /// Complete the pending attended transfer (REFER with Replaces).
    pub fn attended_transfer_exec(&self) {
        self.phone.attended_transfer_exec();
    }
    /// Abort the pending attended transfer.
    pub fn attended_transfer_abort(&self) {
        self.phone.attended_transfer_abort();
    }
}

/// Merge newly-parsed inbound INVITE headers into the agent state, notifying
/// watchers only when a new Call-ID appears (so the `await_until` wait
/// loops aren't woken on every poll). The incremental trace reading lives in
/// [`siptrace::TraceTail`]; this is just the state-merge policy.
fn merge_received_headers(state_tx: &watch::Sender<AgentState>, invites: siptrace::InviteHeaders) {
    if invites.is_empty() {
        return;
    }
    let has_new = {
        let cur = state_tx.borrow();
        invites
            .keys()
            .any(|k| !cur.received_headers.contains_key(k))
    };
    if has_new {
        state_tx.send_modify(|s| {
            for (call_id, headers) in invites {
                s.received_headers.entry(call_id).or_insert(headers);
            }
        });
    }
}

async fn connect_retry(port: u16, within: Duration) -> Result<TcpStream> {
    let deadline = tokio::time::Instant::now() + within;
    loop {
        match TcpStream::connect(("127.0.0.1", port)).await {
            Ok(s) => return Ok(s),
            Err(e) if tokio::time::Instant::now() >= deadline => {
                return Err(e).context("ctrl_tcp connect timed out");
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}
