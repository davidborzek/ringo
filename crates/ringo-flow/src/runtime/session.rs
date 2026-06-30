//! A live agent session: one `ringo-flow agent` worker process plus its event
//! stream. Incoming events are folded into an [`AgentState`] (published over a
//! `watch` channel) that the runner asserts against. Every agent runs in its own
//! process (see the `ringo-agent` crate); the parent holds only a [`ProcessClient`].

use super::state::{AgentState, reduce};
use anyhow::{Context, Result};
use ringo_agent::audio::ToneAnalysis;
use ringo_agent::{AgentConfig, ProcessClient};
use ringo_core::account::{Account, BackendOptions};
use ringo_core::event::AppEvent;
use ringo_core::event::InviteHeaders;
use ringo_core::event::MediaStats;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// How often the reader task polls for inbound INVITE headers.
const TRACE_POLL_INTERVAL: Duration = Duration::from_millis(150);

pub struct AgentSession {
    pub aor: String,
    pub regint: u32,
    client: ProcessClient,
    state_rx: watch::Receiver<AgentState>,
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

    /// Goertzel analysis of the last `window` of received audio, computed in the
    /// worker on its in-process captured buffer (used by `verify-audio`).
    pub fn analyze_tone(&self, freq: u32, window: Duration) -> ToneAnalysis {
        self.client.analyze_tone(freq, window)
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
