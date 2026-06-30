//! A live agent session: one `ringo-flow agent` worker process plus its event
//! stream. Incoming events are folded into an [`AgentState`] (published over a
//! `watch` channel) that the runner asserts against. Every agent runs in its own
//! process (see the `ringo-agent` crate); the parent holds only a [`ProcessClient`].

use super::state::{AgentState, reduce};
use anyhow::{Context, Result};
use ringo_agent::audio::{self, ToneAnalysis};
use ringo_agent::{AgentConfig, ProcessClient};
use ringo_core::account::{Account, BackendOptions};
use ringo_core::event::AppEvent;
use ringo_core::event::InviteHeaders;
use ringo_core::event::MediaStats;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;

/// How often the reader task polls for inbound INVITE headers.
const TRACE_POLL_INTERVAL: Duration = Duration::from_millis(150);

/// Cap on the parent-side received-audio ring buffer (~30 s at 48 kHz). The tone
/// analysis only looks at the last `window`, so we keep a bounded recent tail and
/// drop the oldest samples — long scenarios can't grow the buffer unboundedly.
const RX_BUFFER_CAP_SAMPLES: usize = 48_000 * 30;

/// Parent-side accumulator for an agent's streamed received audio. Filled by a
/// drain thread reading the `start_rx_audio` channel; read by `analyze_tone`.
#[derive(Default)]
struct RxBuffer {
    samples: Vec<i16>,
    rate: u32,
}

pub struct AgentSession {
    pub aor: String,
    pub regint: u32,
    client: ProcessClient,
    state_rx: watch::Receiver<AgentState>,
    /// Received-audio ring buffer; `None` until the first `analyze_tone` starts
    /// the RX stream (lazy — only agents we actually verify pay the cost).
    rx_audio: Mutex<Option<Arc<Mutex<RxBuffer>>>>,
}

impl AgentSession {
    /// Spawn a worker process for an account, connect, and fold events into
    /// shared state. `name` labels the instance/logs.
    pub async fn connect(name: &str, account: Account, options: &BackendOptions) -> Result<Self> {
        let config = agent_config(name, &account, options);
        let (client, events) =
            ProcessClient::spawn(config).with_context(|| format!("spawn agent `{name}`"))?;

        let (state_tx, state_rx) = watch::channel(AgentState::default());
        let state_tx = Arc::new(state_tx);

        // Bridge the sync mpsc events into an async channel so the reader task
        // can consume them without blocking the tokio runtime.
        let (async_event_tx, mut async_event_rx) = tokio::sync::mpsc::channel::<AppEvent>(64);
        tokio::task::spawn_blocking(move || {
            while let Ok(event) = events.recv() {
                if async_event_tx.blocking_send(event).is_err() {
                    break;
                }
            }
        });

        // Reader: events → state.
        let reader_tx = Arc::clone(&state_tx);
        tokio::spawn(async move {
            while let Some(event) = async_event_rx.recv().await {
                reader_tx.send_modify(|s| reduce(s, &event));
            }
        });

        // Trace poll: inbound INVITE headers → state (the events don't carry
        // them). The worker pushes them; we drain the client's buffer here.
        let headers = client.headers_handle();
        let trace_tx = Arc::clone(&state_tx);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(TRACE_POLL_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if trace_tx.is_closed() {
                    break;
                }
                let new = headers.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(invites) = new {
                    merge_received_headers(&trace_tx, invites);
                }
            }
        });

        Ok(Self {
            aor: format!("sip:{}@{}", account.username, account.domain),
            regint: account.regint.unwrap_or(3600),
            client,
            state_rx,
            rx_audio: Mutex::new(None),
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

    /// Switch the agent's audio source on its active call.
    pub fn set_audio_source(&self, spec: &str) {
        self.client.set_audio_source(spec);
    }

    /// Goertzel analysis of the last `window` of received audio (used by
    /// `verify-audio`). The worker streams raw received PCM over the agent proto;
    /// the analysis runs here, on the parent, over the streamed tail. The RX
    /// stream is started lazily on the first call (so unverified agents don't pay
    /// for it) — that's fine because `verify-audio` sleeps `window` right after,
    /// leaving time for the stream to spin up and fill.
    pub fn analyze_tone(&self, freq: u32, window: Duration) -> ToneAnalysis {
        let buf = self.rx_audio_buffer();
        let g = buf.lock().unwrap_or_else(|e| e.into_inner());
        if g.rate == 0 {
            // No audio frames yet (stream just started / media not flowing).
            return ToneAnalysis::default();
        }
        audio::analyze_tone_samples(&g.samples, g.rate, freq, window)
    }

    /// Start the RX audio stream now (idempotent) so the buffer is already
    /// filling before `verify-audio` sleeps its first window — otherwise the
    /// first analysis runs against an empty buffer and wastes a whole window.
    ///
    /// Also clears any buffered history so each verify analyses only audio
    /// captured from here on: the stream lives for the agent's whole lifetime,
    /// so without this a tone from a previous call/leg could still sit in the
    /// window tail and produce a false positive (the old worker-side buffer was
    /// reset per call). Safe because `verify-audio` sleeps a full window after
    /// priming, refilling the tail before it analyses.
    pub fn prime_received_audio(&self) {
        let buf = self.rx_audio_buffer();
        let mut g = buf.lock().unwrap_or_else(|e| e.into_inner());
        g.samples.clear();
    }

    /// The agent's received-audio buffer, starting the RX stream + drain thread on
    /// first use. Subsequent calls reuse the same buffer.
    fn rx_audio_buffer(&self) -> Arc<Mutex<RxBuffer>> {
        let mut slot = self.rx_audio.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(buf) = slot.as_ref() {
            return Arc::clone(buf);
        }
        let buf = Arc::new(Mutex::new(RxBuffer::default()));
        let rx = self.client.start_rx_audio();
        let drain = Arc::clone(&buf);
        // Detached: the channel closes when the worker/client drops the RX sender
        // (teardown), which ends the iterator and the thread.
        std::thread::spawn(move || {
            for frame in rx {
                let mut g = drain.lock().unwrap_or_else(|e| e.into_inner());
                g.rate = frame.rate;
                g.samples.extend_from_slice(&frame.samples);
                if g.samples.len() > RX_BUFFER_CAP_SAMPLES {
                    let excess = g.samples.len() - RX_BUFFER_CAP_SAMPLES;
                    g.samples.drain(0..excess);
                }
            }
        });
        *slot = Some(Arc::clone(&buf));
        buf
    }

    /// Ask the worker to write its captured sent/received audio as WAVs named
    /// `<prefix>-sent.wav` / `<prefix>-recv.wav`; returns the paths written.
    pub fn save_audio(&self, prefix: &str) -> Vec<String> {
        self.client.save_audio(prefix)
    }

    /// Number of active calls in the worker (used by teardown's BYE-flush wait).
    pub fn call_count(&self) -> u32 {
        self.client.call_count()
    }

    /// Signal the worker to deregister and exit (non-blocking). Call on all
    /// agents before dropping them so the de-REGISTERs run concurrently.
    pub fn request_shutdown(&self) {
        self.client.request_shutdown();
    }

    /// RTP media stats (jitter/loss/RTT + MOS) for the active or last call.
    pub fn media_stats(&self) -> Option<MediaStats> {
        self.client.media_stats()
    }

    /// DTMF digits received on the active/last call so far, in order.
    pub fn received_dtmf(&self) -> String {
        self.client.received_dtmf()
    }

    // ── Commands ────────────────────────────────────────────────────────────

    pub fn register(&self) {
        self.client.register(&self.aor, self.regint);
    }
    pub fn dial(&self, target: &str) {
        self.client.dial(target);
    }
    pub fn accept(&self) {
        self.client.accept();
    }
    pub fn hold(&self) {
        self.client.hold();
    }
    pub fn resume(&self) {
        self.client.resume();
    }
    pub fn mute(&self) {
        self.client.mute();
    }
    pub fn send_dtmf(&self, digit: char) {
        self.client.send_dtmf(digit);
    }
    pub fn add_header(&self, key: &str, value: &str) {
        self.client.add_header(key, value);
    }
    pub fn hangup(&self) {
        self.client.hangup();
    }
    pub fn hangup_all(&self) {
        self.client.hangup_all();
    }
    pub fn transfer(&self, uri: &str) {
        self.client.transfer(uri);
    }
    pub fn attended_transfer_start(&self, uri: &str) {
        self.client.attended_transfer_start(uri);
    }
    pub fn attended_transfer_exec(&self) {
        self.client.attended_transfer_exec();
    }
    pub fn attended_transfer_abort(&self) {
        self.client.attended_transfer_abort();
    }
    pub fn deflect_incoming(&self, contact: &str, diversion: Option<&str>) {
        self.client.deflect_incoming(contact, diversion);
    }
    pub fn arm_invite_response(&self, scode: u16, reason: &str, headers: Vec<String>) {
        self.client.arm_invite_response(scode, reason, headers);
    }
    pub fn disarm_invite_response(&self) {
        self.client.disarm_invite_response();
    }
}

/// Build the worker handshake config from an account + backend options. Process
/// agents always register as `catchall` — there is exactly one UA per worker
/// process, so the catch-all fallback is unambiguous (see the worker).
fn agent_config(name: &str, account: &Account, options: &BackendOptions) -> AgentConfig {
    let mut account = account.clone();
    account.catchall = true;
    AgentConfig {
        name: name.to_string(),
        account,
        options: options.clone(),
    }
}

/// Merge newly-parsed inbound INVITE headers into the agent state, notifying
/// watchers only when a new Call-ID appears (so the `await_until` wait loops
/// aren't woken on every poll).
fn merge_received_headers(state_tx: &watch::Sender<AgentState>, invites: InviteHeaders) {
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
