//! The agent hub: the configured agents and their (lazily spawned) worker
//! processes.
//!
//! Agents are **not** started when the MCP server starts — a client that never
//! places a call shouldn't hold SIP registrations. Instead each agent's worker
//! process (one baresip UA, own SIP port and registration) is spawned on the
//! first tool call that touches it, with per-agent single-flight so concurrent
//! calls don't double-spawn. A worker that died (crash/exit) is respawned on
//! the next tool call the same way.
//!
//! Events from each worker are folded into a per-agent [`AgentState`] and
//! published on a broadcast channel so any number of concurrent `wait_event`
//! tool calls can observe them.

use crate::bridge::{BridgeState, StreamInfo, StreamMode};
use crate::config::LoadedConfig;
use crate::headers::{HeaderContext, HeaderTemplate};
use crate::state::{AgentState, reduce};
use anyhow::{Context, Result};
use regex::Regex;
use ringo_agent::{AgentConfig, ProcessClient};
use ringo_core::AudioFrame;
use ringo_core::account::{Account, BackendOptions};
use ringo_core::event::AppEvent;
use std::net::{IpAddr, SocketAddr};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, broadcast, mpsc};

/// Capacity of the per-agent event broadcast channel. Events that nobody is
/// waiting for are dropped (the state fold still sees them); `wait_event`
/// callers that fall behind get a lag notification, not stale data.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// How often the bridge polls the worker for newly seen inbound INVITE headers
/// (same cadence as ringo-flow's trace poll).
const HEADER_POLL_INTERVAL: Duration = Duration::from_millis(150);

/// Capacity of the per-agent received-audio broadcast (the WS bridge fan-out).
/// A frame ≈ 20 ms → a lagging consumer is dropped up to ~2.5 s behind and gets
/// an `rx_lagged` notice instead of stale data.
const RX_TAP_CAPACITY: usize = 128;

/// Messages for an agent's single TX writer thread (see `Agent::tx_channel`).
/// One channel serializes both kinds so the worker's same-thread sequencing
/// rule (StartTxAudio before any audio frame) always holds.
#[derive(Debug)]
pub(crate) enum TxMsg {
    /// (Re-)arm the worker's streamed source at `rate` Hz. Re-arming flushes
    /// whatever was still queued (barge-in).
    Start { rate: u32 },
    /// Mono s16 PCM into the call.
    Audio(Vec<i16>),
}

/// The global dial policy (`--dial-allow` / `--dial-deny` CLI flags):
/// restrictions on every target the `dial`/`transfer` tools may place a call
/// to — the LLM agent must not be able to dial freely (expensive
/// destinations, premium numbers, …).
///
/// Regexes match the dialed number (the user part of the resolved target)
/// and the full resolved URI — whichever matches counts. Empty =
/// unrestricted.
#[derive(Debug, Clone, Default)]
pub struct DialPolicy {
    deny: Vec<Regex>,
    allow: Vec<Regex>,
}

impl DialPolicy {
    /// Compile the flag values; invalid regexes fail the startup.
    pub fn build(deny: Vec<String>, allow: Vec<String>) -> Result<Self> {
        let compile = |rules: Vec<String>, what: &str| -> Result<Vec<Regex>> {
            rules
                .into_iter()
                .map(|r| {
                    Regex::new(&r)
                        .with_context(|| format!("--dial-{what} rule `{r}` is not a valid regex"))
                })
                .collect()
        };
        Ok(Self {
            deny: compile(deny, "deny")?,
            allow: compile(allow, "allow")?,
        })
    }

    /// Check a resolved target (`sip:<user>@<host>` or a full URI): `Ok` if
    /// permitted, `Err` naming the violated rule.
    pub fn check(&self, resolved: &str) -> std::result::Result<(), String> {
        let user = resolved
            .split_once(':')
            .and_then(|(_, rest)| rest.split_once('@').map(|(u, _)| u.to_string()))
            .unwrap_or_else(|| resolved.to_string());
        let user = user.as_str();
        for rule in &self.deny {
            if rule.is_match(user) || rule.is_match(resolved) {
                return Err(format!(
                    "denied by dial policy (matched deny rule `{}`)",
                    rule.as_str()
                ));
            }
        }
        if !self.allow.is_empty()
            && !self
                .allow
                .iter()
                .any(|r| r.is_match(user) || r.is_match(resolved))
        {
            return Err("denied by dial policy (no allow rule matches)".to_string());
        }
        Ok(())
    }
}

/// All configured agents, addressed by name. Spawn-on-demand; see the module
/// docs. Cheap to build (`Hub::new` validates nothing, spawns nothing).
pub struct Hub {
    slots: Vec<Slot>,
    /// The global dial policy ([dial] in the config) every `dial`/`transfer`
    /// passes before reaching the worker.
    dial: DialPolicy,
    /// Loopback host the live-audio WS bridge binds to (ephemeral port).
    bridge_host: IpAddr,
    /// The lazily-started WS bridge (see the `bridge` module); `None` until
    /// the first `stream_open`.
    bridge: AsyncMutex<Option<Arc<BridgeState>>>,
}

/// One configured agent and its (optional) running instance. The async mutex
/// is the single-flight spawn lock for this agent: the first tool call holds
/// it while spawning; concurrent calls for the same agent wait and then see
/// the already-running instance.
struct Slot {
    name: String,
    aor: String,
    account: Account,
    options: BackendOptions,
    custom_headers: Vec<(String, String)>,
    agent: AsyncMutex<Option<Arc<Agent>>>,
}

impl Hub {
    /// Build the hub from a (already validated) config and the runtime CLI
    /// knobs (the config is agents-only). Spawns nothing.
    pub fn new(config: LoadedConfig, dial: DialPolicy, bridge_host: IpAddr) -> Self {
        let slots = config
            .agents
            .into_iter()
            .map(|def: crate::config::AgentDef| {
                let aor = format!("sip:{}@{}", def.account.username, def.account.domain);
                Slot {
                    name: def.name,
                    aor,
                    options: config.backend.clone(),
                    custom_headers: def.custom_headers,
                    agent: AsyncMutex::new(None),
                    account: def.account,
                }
            })
            .collect();
        Self {
            slots,
            dial,
            bridge_host,
            bridge: AsyncMutex::new(None),
        }
    }

    /// All configured agents with their current liveness, in config order.
    /// Does NOT spawn anything — a pure view over config + running state.
    pub fn overview(&self) -> Vec<AgentOverview> {
        self.slots
            .iter()
            .map(|s| {
                let (running, state) = match s.agent.try_lock() {
                    // Holding the lock = a spawn is in flight: running as far
                    // as callers are concerned (it will be there momentarily).
                    Err(_) => (true, None),
                    Ok(guard) => match guard.as_ref() {
                        Some(a) => (true, Some(a.state())),
                        None => (false, None),
                    },
                };
                AgentOverview {
                    name: s.name.clone(),
                    aor: s.aor.clone(),
                    running,
                    state,
                }
            })
            .collect()
    }

    /// Get the agent `name`, spawning its worker process if it isn't running
    /// (first use, or the previous worker died). Concurrent callers for the
    /// same agent share one spawn (single-flight); different agents spawn
    /// independently and in parallel.
    pub async fn get(&self, name: &str) -> Result<Arc<Agent>> {
        let slot = self
            .slots
            .iter()
            .find(|s| s.name == name)
            .with_context(|| {
                let known: Vec<&str> = self.slots.iter().map(|s| s.name.as_str()).collect();
                format!("unknown agent `{name}`; known agents: {}", known.join(", "))
            })?;

        // The per-slot lock IS the single-flight: the first caller through
        // spawns while holding it; everyone else parks here and then finds the
        // instance already in place.
        let mut guard = slot.agent.lock().await;
        if let Some(agent) = guard.as_ref() {
            if !agent.is_dead() {
                return Ok(Arc::clone(agent));
            }
            eprintln!("ringo-mcp: agent `{name}` worker is gone; respawning");
        }
        let agent = Arc::new(
            Agent::connect(
                &slot.name,
                slot.account.clone(),
                slot.options.clone(),
                &slot.custom_headers,
            )
            .await
            .with_context(|| format!("start agent `{}`", slot.name))?,
        );
        *guard = Some(Arc::clone(&agent));
        Ok(agent)
    }

    /// Resolve `target` for `agent` and check it against the global dial
    /// policy. Returns the resolved URI on success — the caller dials that.
    pub fn check_dial(&self, target: &str, domain: &str) -> std::result::Result<String, String> {
        let resolved = resolve_target(target, domain);
        self.dial.check(&resolved)?;
        Ok(resolved)
    }

    // ── live-audio bridge ───────────────────────────────────────────────

    /// Open a live-audio stream for `agent`: mints a one-shot, TTL-bound token
    /// for the WS bridge, primes the agent's RX tap / TX writer as the mode
    /// requires, and returns the `ws://` URL plus the stream id.
    pub async fn stream_open(
        self: &Arc<Self>,
        agent: &str,
        mode: StreamMode,
        tx_rate: u32,
    ) -> Result<StreamInfo> {
        let agent = self.get(agent).await?;
        // Prime the taps before minting the URL, so the rate is settled by the
        // time the client connects (rx) and the writer exists before any
        // inbound audio can race the arming (tx).
        if mode != StreamMode::Tx {
            agent.rx_frames().await;
        }
        if mode != StreamMode::Rx {
            agent.tx_channel().await;
        }

        let bridge = self.ensure_bridge().await?;
        bridge.mint_grant(agent.name.clone(), mode, tx_rate).await
    }

    /// Stop a running agent: its worker deregisters and exits. Returns whether
    /// a worker was actually stopped (`false` = the agent wasn't running).
    /// The agent starts again on its next use — stopping is the counterpart to
    /// the lazy start, not a disable.
    pub async fn stop(&self, name: &str) -> Result<bool> {
        let slot = self
            .slots
            .iter()
            .find(|s| s.name == name)
            .with_context(|| {
                let known: Vec<&str> = self.slots.iter().map(|s| s.name.as_str()).collect();
                format!("unknown agent `{name}`; known agents: {}", known.join(", "))
            })?;
        let mut guard = slot.agent.lock().await;
        let Some(agent) = guard.take() else {
            return Ok(false);
        };
        // Ask the worker to deregister and exit (non-blocking); the slot is
        // already empty, so the next use spawns a fresh worker.
        agent.shutdown_worker();
        // Dropping the last Arc reaps the child (waits up to the shutdown
        // grace) — off the async threads. Live WS connections keep their Arc
        // a moment longer; their health check closes them once the worker is
        // gone, releasing the final reference.
        tokio::task::spawn_blocking(move || drop(agent));
        Ok(true)
    }

    /// Close an open stream by id (the token from `stream_open`).
    pub async fn stream_close(&self, stream_id: &str) -> Result<()> {
        let bridge = self
            .bridge
            .lock()
            .await
            .as_ref()
            .map(Arc::clone)
            .context("no stream has ever been opened (bridge not running)")?;
        bridge.close_stream(stream_id)
    }

    /// Ask every running worker to deregister and exit (see `ProcessClient`'s
    /// shutdown budget). Called after the MCP client disconnected: the workers'
    /// event streams then close, which ends the per-agent poll tasks and lets
    /// the `ProcessClient` drops reap the children. Idempotent.
    pub fn shutdown(&self) {
        for slot in &self.slots {
            // The lock is only held briefly by idle paths (`get` releases it
            // after the spawn); a spawn in flight is asking for a worker that
            // will be told to shut down by its own Drop when it lands — or is
            // already gone by then, which the worker handles gracefully.
            if let Ok(guard) = slot.agent.try_lock() {
                if let Some(agent) = guard.as_ref() {
                    agent.shutdown_worker();
                }
            }
        }
    }

    /// Start the WS bridge listener (once) if it isn't running yet.
    async fn ensure_bridge(self: &Arc<Self>) -> Result<Arc<BridgeState>> {
        let mut guard = self.bridge.lock().await;
        if let Some(b) = guard.as_ref() {
            return Ok(Arc::clone(b));
        }
        let listener = tokio::net::TcpListener::bind((self.bridge_host, 0))
            .await
            .with_context(|| format!("bind WS bridge on {}", self.bridge_host))?;
        let addr: SocketAddr = listener.local_addr()?;
        let state = Arc::new(BridgeState::new(addr));
        *guard = Some(Arc::clone(&state));

        // The accept loop must not keep the hub (and its workers) alive after
        // the server exits — hence the Weak, upgraded per connection.
        let weak = Arc::downgrade(self);
        let accept_state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                let (stream, _peer) = match listener.accept().await {
                    Ok(v) => v,
                    Err(_) => break,
                };
                let Some(hub) = weak.upgrade() else { break };
                crate::bridge::accept(accept_state.clone(), hub, stream);
            }
        });
        eprintln!("ringo-mcp: live-audio bridge listening on ws://{addr}/s/<token>");
        Ok(state)
    }
}

/// One configured agent as seen by `list_agents`: config facts plus, if the
/// worker is running, its reduced state.
pub struct AgentOverview {
    /// Config label (tool-facing name).
    pub name: String,
    /// `sip:username@domain` of the configured AOR.
    pub aor: String,
    /// `false` = configured but never used since server start (or dead and not
    /// yet respawned).
    pub running: bool,
    /// Reduced state, `None` unless the worker is running.
    pub state: Option<AgentState>,
}

/// Whether an event is worth surfacing to an MCP agent. Everything ringo-core
/// decodes into a named, backend-neutral `AppEvent` is relevant; `Unknown` (raw,
/// backend-specific events — SDP negotiation, RTP/RTCP mechanics, lifecycle
/// internals, periodic reports) never is: their outcomes arrive as named
/// events, and backend details don't belong in the agent-facing surface.
/// Debugging the raw stream: `RINGO_AGENT_LOG` / SIP trace.
fn is_agent_relevant(event: &AppEvent) -> bool {
    !matches!(event, AppEvent::Unknown { .. })
}

/// One connected agent: its worker-process handle plus reduced state and a
/// live event feed.
pub struct Agent {
    /// Config label (tool-facing name).
    pub name: String,
    /// `sip:username@domain` of the registered AOR.
    pub aor: String,
    /// The agent's SIP domain, used to resolve bare dial targets.
    pub domain: String,
    client: Arc<ProcessClient>,
    state: Arc<Mutex<AgentState>>,
    events: broadcast::Sender<AppEvent>,
    /// The agent's single received-audio tap, lazily started by
    /// [`Agent::rx_frames`] (the worker's tap is single-sink — this Agent owns
    /// the one subscription and fans out from here).
    rx_tap: AsyncMutex<Option<broadcast::Sender<AudioFrame>>>,
    /// The agent's single TX writer channel, lazily created by
    /// [`Agent::tx_channel`] (same-thread sequencing — see `TxMsg`).
    tx: AsyncMutex<Option<mpsc::Sender<TxMsg>>>,
    /// Config-declared custom-header templates for outgoing INVITEs: static
    /// ones were applied once at connect, dynamic ones are re-rendered per
    /// [`Agent::dial`] (fresh `${uuid}` each call).
    custom_headers: Vec<(String, HeaderTemplate)>,
}

impl Agent {
    /// Spawn the worker process and connect its event stream (mirrors
    /// ringo-flow's `AgentSession::connect`). Blocks until the worker's
    /// readiness handshake (sub-second; bounded by the client's ready timeout).
    /// Static (non-template) custom headers are applied to the worker here.
    async fn connect(
        name: &str,
        account: Account,
        options: BackendOptions,
        custom_headers: &[(String, String)],
    ) -> Result<Self> {
        let custom_headers: Vec<(String, HeaderTemplate)> = custom_headers
            .iter()
            .map(|(k, v)| (k.clone(), HeaderTemplate::new(v)))
            .collect();
        let aor = format!("sip:{}@{}", account.username, account.domain);
        let domain = account.domain.clone();
        let config = AgentConfig {
            name: name.to_string(),
            account,
            options,
        };
        let label = name.to_string();
        let (client, events) = tokio::task::spawn_blocking(move || ProcessClient::spawn(config))
            .await
            .context("agent spawn task panicked")?
            .with_context(|| format!("spawn worker for `{label}`"))?;
        let client = Arc::new(client);

        let (events_tx, _) = broadcast::channel::<AppEvent>(EVENT_CHANNEL_CAPACITY);
        let state = Arc::new(Mutex::new(AgentState::default()));

        // Bridge the worker's blocking std channel into the async world: reduce
        // into the shared state, fan out to any wait_event subscribers, poll for
        // inbound INVITE headers between events, and reset a stale `play` spec
        // when the agent's last call ends.
        let bridge_state = Arc::clone(&state);
        let bridge_tx = events_tx.clone();
        let bridge_client = Arc::clone(&client);
        let headers = client.headers_handle();
        tokio::task::spawn_blocking(move || {
            loop {
                match events.recv_timeout(HEADER_POLL_INTERVAL) {
                    Ok(event) => {
                        // Unmapped baresip events surface only if an agent can
                        // act on them (see `is_agent_relevant`); the state fold
                        // ignores Unknown either way.
                        if !is_agent_relevant(&event) {
                            continue;
                        }
                        let _ = bridge_tx.send(event.clone());
                        let mut g = bridge_state.lock().unwrap_or_else(|e| e.into_inner());
                        reduce(&mut g, &event);
                        // `play` is call-scoped for MCP agents: when the agent's LAST
                        // call ended, stop transmitting the stale spec so the next
                        // call doesn't replay it (the underlying ausrc is per-UA and
                        // would otherwise persist, as ringo-flow scenarios want).
                        if closes_last_call(&event, &g) {
                            bridge_client.set_audio_source("silence");
                        }
                    }
                    // Poll window: pick up INVITE headers the worker collected.
                    Err(RecvTimeoutError::Timeout) => {
                        let invites = headers.lock().unwrap_or_else(|e| e.into_inner()).take();
                        if let Some(invites) = invites {
                            let mut g = bridge_state.lock().unwrap_or_else(|e| e.into_inner());
                            g.merge_invites(invites);
                        }
                    }
                    // Worker event stream closed: the worker exited (or crashed).
                    Err(RecvTimeoutError::Disconnected) => {
                        let mut g = bridge_state.lock().unwrap_or_else(|e| e.into_inner());
                        g.worker_dead = true;
                        return;
                    }
                }
            }
        });

        eprintln!("ringo-mcp: agent `{name}` ready ({aor})");
        // Static headers once at startup; dynamic templates (e.g. `${uuid}`)
        // are re-added per call by `dial` so each call gets a fresh value.
        for (key, tpl) in &custom_headers {
            if !tpl.is_dynamic() {
                client.add_header(key, tpl.raw());
            }
        }
        Ok(Self {
            name: name.to_string(),
            aor,
            domain,
            client,
            state,
            events: events_tx,
            rx_tap: AsyncMutex::new(None),
            tx: AsyncMutex::new(None),
            custom_headers,
        })
    }

    /// Whether the worker's event stream closed (worker exit/crash). A dead
    /// agent stays dead until the hub respawns it on the next tool call.
    pub fn is_dead(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .worker_dead
    }

    /// A snapshot of the reduced state (registration, calls, last close).
    pub fn state(&self) -> AgentState {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Wait for the agent's next event, or `None` on timeout. Subscribes at
    /// call time — events that happened before are not replayed (poll
    /// `agent_status` for state instead).
    ///
    /// With `names`, events that don't match one of the given names are
    /// skipped (still folded into the agent state and visible to other
    /// waiters) — `None` means any event.
    pub async fn wait_event(&self, timeout: Duration, names: Option<&[&str]>) -> Option<AppEvent> {
        let mut rx = self.events.subscribe();
        tokio::time::timeout(timeout, async {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if names.is_none_or(|n| n.contains(&crate::event_name(&event))) {
                            return Some(event);
                        }
                    }
                    // Fell behind the ring: skip the notice, next event follows.
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        })
        .await
        .ok()
        .flatten()
    }

    /// Subscribe to this agent's live event feed (the WS bridge's push
    /// channel; same stream `wait_event` consumes).
    pub fn subscribe_events(&self) -> broadcast::Receiver<AppEvent> {
        self.events.subscribe()
    }

    /// Subscribe to the agent's received audio (mono s16 [`AudioFrame`]s).
    /// Starts the worker's RX stream lazily, exactly once per agent — the
    /// worker's tap is single-sink, so this Agent owns the one tap and fans
    /// out to any number of subscribers from here.
    pub async fn rx_frames(&self) -> broadcast::Receiver<AudioFrame> {
        let mut guard = self.rx_tap.lock().await;
        if let Some(tap) = guard.as_ref() {
            return tap.subscribe();
        }
        let (tap, _) = broadcast::channel(RX_TAP_CAPACITY);
        let rx = self.client.start_rx_audio();
        let forwarder = tap.clone();
        tokio::task::spawn_blocking(move || {
            // No subscriber ≠ stop: the worker has no StopRxAudio, and a new
            // subscriber may arrive any time. Frames are dropped by the
            // broadcast itself when nobody listens.
            while let Ok(frame) = rx.recv() {
                let _ = forwarder.send(frame);
            }
            // Worker gone: drop the tap so subscribers see the channel close.
            drop(forwarder);
        });
        *guard = Some(tap.clone());
        tap.subscribe()
    }

    /// The agent's TX channel: a single writer thread serializes `Start`/`Audio`
    /// onto the worker's stdin (the worker requires StartTxAudio before any
    /// audio frame, and the stdin lock is not FIFO across threads — one
    /// thread does both, in order). Created lazily, once per agent.
    pub(crate) async fn tx_channel(&self) -> mpsc::Sender<TxMsg> {
        let mut guard = self.tx.lock().await;
        if let Some(tx) = guard.as_ref() {
            return tx.clone();
        }
        let (tx, mut rx) = mpsc::channel::<TxMsg>(256);
        let client = Arc::clone(&self.client);
        tokio::task::spawn_blocking(move || {
            while let Some(msg) = rx.blocking_recv() {
                match msg {
                    TxMsg::Start { rate } => client.start_tx_audio(rate),
                    TxMsg::Audio(samples) => client.push_tx_audio(&samples),
                }
            }
            // All senders dropped (agent gone): the thread ends, releasing its
            // ProcessClient reference so the worker can be reaped.
        });
        *guard = Some(tx.clone());
        tx
    }

    /// Dial the (already policy-checked, resolved) target URI, re-rendering
    /// dynamic header templates first (fresh `${uuid}` per call).
    pub fn dial(&self, resolved: &str) {
        self.refresh_dynamic_headers();
        self.client.dial(resolved);
    }

    /// Re-render the dynamic custom headers for the next outgoing INVITE.
    /// `uarmheader` removes *all* headers of a name, so for any key with a
    /// dynamic template we remove and re-add *every* header of that key —
    /// including static ones declared for the same key (same approach as
    /// ringo-phone, so duplicate History-Info-style entries survive).
    fn refresh_dynamic_headers(&self) {
        let dynamic_keys: std::collections::HashSet<&str> = self
            .custom_headers
            .iter()
            .filter(|(_, tpl)| tpl.is_dynamic())
            .map(|(key, _)| key.as_str())
            .collect();
        if dynamic_keys.is_empty() {
            return;
        }
        let ctx = HeaderContext::for_call();
        for key in &dynamic_keys {
            self.client.rm_header(key);
        }
        for (key, tpl) in &self.custom_headers {
            if dynamic_keys.contains(key.as_str()) {
                self.client.add_header(key, &tpl.render(&ctx));
            }
        }
    }

    /// Add (or overwrite) a custom header on the agent's outgoing INVITEs.
    /// The value is a template rendered once, now — e.g. `"session-${uuid}"` gets
    /// a fresh uuid at this call, not per future call.
    pub fn add_header(&self, name: &str, value: &str) {
        let rendered = HeaderTemplate::new(value).render(&HeaderContext::for_call());
        // baresip appends on add — remove the name first so add_header
        // REPLACES (a second call with the same name must not stack duplicates).
        self.client.rm_header(name);
        self.client.add_header(name, &rendered);
    }

    /// Remove ALL headers with this name from the agent's outgoing INVITEs.
    pub fn rm_header(&self, name: &str) {
        self.client.rm_header(name);
    }

    /// Accept the currently ringing (incoming) call.
    pub fn accept(&self) {
        self.client.accept();
    }

    /// Hang up the current call.
    pub fn hangup(&self) {
        self.client.hangup();
    }

    /// Hang up all calls.
    pub fn hangup_all(&self) {
        self.client.hangup_all();
    }

    /// Put the current call on hold. The phase is set optimistically:
    /// baresip only reports PEER hold/resume as events, so without this the
    /// agent's own hold would never show up in `agent_status`.
    pub fn hold(&self) {
        self.client.hold();
        self.set_current_phase(crate::state::CallPhase::Held);
    }

    /// Resume the held call (see `hold` for the optimistic phase).
    pub fn resume(&self) {
        self.client.resume();
        self.set_current_phase(crate::state::CallPhase::Established);
    }

    /// Optimistically set the phase of the most recent (current) call.
    fn set_current_phase(&self, phase: crate::state::CallPhase) {
        let mut g = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(call) = g.calls.last_mut() {
            call.phase = phase;
        }
    }

    /// Mute the outgoing audio (remote party hears silence).
    pub fn mute(&self) {
        self.client.mute();
    }

    /// Send one DTMF digit on the current call.
    pub fn send_dtmf(&self, digit: char) {
        self.client.send_dtmf(digit);
    }

    /// Blind-transfer the current call to the (policy-checked, resolved)
    /// target URI.
    pub fn transfer(&self, resolved: &str) {
        self.client.transfer(resolved);
    }

    /// Set the audio the agent transmits: `"silence"`, `"ausine,<freq>"` (a
    /// tone) or `"aufile,<path>"` (a WAV file). See ringo's ausrc module.
    /// Call-scoped: automatically resets to `silence` once the agent's last
    /// call has ended, so the next call doesn't replay it.
    pub fn play(&self, spec: &str) {
        self.client.set_audio_source(spec);
    }

    /// RTP quality stats of the current call, if a call is up.
    pub fn media_stats(&self) -> Option<ringo_core::event::MediaStats> {
        self.client.media_stats()
    }

    /// DTMF digits received so far on the current call.
    pub fn received_dtmf(&self) -> String {
        self.client.received_dtmf()
    }

    /// Ask this agent's worker to deregister and exit (server teardown).
    pub fn shutdown_worker(&self) {
        self.client.request_shutdown();
    }

    /// Number of currently active calls.
    pub fn call_count(&self) -> u32 {
        self.client.call_count()
    }

    /// Write the call's sent+received audio to WAV files under `prefix`;
    /// returns the created paths. Requires `record_audio` in `[backend]`.
    pub fn save_audio(&self, prefix: &str) -> Vec<String> {
        self.client.save_audio(prefix)
    }
}

/// Resolve a dial/transfer target: full URIs pass through, `user@host` gets a
/// `sip:` prefix, and a bare number/extension becomes `sip:<target>@<domain>`.
fn resolve_target(target: &str, domain: &str) -> String {
    if target.starts_with("sip:") || target.starts_with("sips:") {
        target.to_string()
    } else if target.contains('@') {
        format!("sip:{target}")
    } else {
        format!("sip:{target}@{domain}")
    }
}

/// Whether this event ended the agent's last remaining call (post-reduce
/// state) — the trigger for resetting a stale `play` spec to silence.
fn closes_last_call(event: &AppEvent, state: &AgentState) -> bool {
    matches!(event, AppEvent::CallClosed { .. }) && state.calls.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_bare_numbers_and_uris() {
        assert_eq!(
            resolve_target("1002", "pbx.example.com"),
            "sip:1002@pbx.example.com"
        );
        assert_eq!(
            resolve_target("1002@other.example.com", "pbx.example.com"),
            "sip:1002@other.example.com"
        );
        assert_eq!(
            resolve_target("sip:1002@other.example.com", "pbx.example.com"),
            "sip:1002@other.example.com"
        );
        assert_eq!(
            resolve_target("sips:1002@other.example.com", "pbx.example.com"),
            "sips:1002@other.example.com"
        );
    }

    fn loaded_config() -> LoadedConfig {
        LoadedConfig {
            agents: vec![
                crate::config::AgentDef {
                    name: "alice".into(),
                    account: Account {
                        username: "1001".into(),
                        domain: "pbx.example.com".into(),
                        password: "pw".into(),
                        ..Default::default()
                    },
                    custom_headers: Vec::new(),
                },
                crate::config::AgentDef {
                    name: "bob".into(),
                    account: Account {
                        username: "1002".into(),
                        domain: "pbx.example.com".into(),
                        password: "pw".into(),
                        ..Default::default()
                    },
                    custom_headers: Vec::new(),
                },
            ],
            backend: BackendOptions::default(),
        }
    }

    #[test]
    fn hub_builds_lazy_and_overview_knows_config_only() {
        let hub = Hub::new(
            loaded_config(),
            DialPolicy::default(),
            std::net::IpAddr::from([127, 0, 0, 1]),
        );
        let overview = hub.overview();
        assert_eq!(overview.len(), 2);
        assert_eq!(overview[0].name, "alice");
        assert_eq!(overview[0].aor, "sip:1001@pbx.example.com");
        assert!(!overview[0].running, "nothing spawned without a tool call");
        assert!(overview[0].state.is_none());
        // Nothing is running, so the state fold never started:
        assert!(hub.slots.iter().all(|s| s.agent.blocking_lock().is_none()));
    }

    #[test]
    fn unknown_events_never_reach_the_agent() {
        // Backend-specific raw events (any class) stay internal.
        assert!(!is_agent_relevant(&AppEvent::Unknown {
            class: "bevent".into(),
            type_: "29".into(),
        }));
        assert!(!is_agent_relevant(&AppEvent::Unknown {
            class: "other".into(),
            type_: "1".into(),
        }));
        // Every named (backend-neutral) event passes.
        assert!(is_agent_relevant(&AppEvent::RegisterOk {
            account: "a".into()
        }));
        assert!(is_agent_relevant(&AppEvent::CallHold {
            call_id: "c".into()
        }));
        assert!(is_agent_relevant(&AppEvent::CallClosed {
            call_id: "c".into(),
            reason: "bye".into(),
            error: false,
        }));
    }

    #[test]
    fn dial_policy_denies_expensive_targets() {
        let policy = DialPolicy::build(
            vec!["^00".into(), "^0900".into()],
            vec![r"^\d{2,5}$".into()],
        )
        .unwrap();

        // Deny wins over allow, matched on the user part of the resolved URI.
        assert!(policy.check("sip:0044164123@pbx.example.com").is_err());
        let err = policy.check("sip:0900123@pbx.example.com").unwrap_err();
        assert!(err.contains("^0900"), "{err}");
        // Allowlist: an internal extension passes, anything else is denied.
        assert!(policy.check("sip:1002@pbx.example.com").is_ok());
        let err = policy.check("sip:1002003@pbx.example.com").unwrap_err();
        assert!(err.contains("no allow rule"), "{err}");
        // A full-URI allow rule (domain scoping) matches the resolved form.
        let domain_only = DialPolicy::build(vec![], vec!["@pbx\\.example\\.com$".into()]).unwrap();
        assert!(domain_only.check("sip:1002@pbx.example.com").is_ok());
        assert!(domain_only.check("sip:1002@other.example.net").is_err());

        // Unrestricted by default; invalid regex fails the build.
        assert!(DialPolicy::default().check("sip:anything@x").is_ok());
        let err = DialPolicy::build(vec!["[".into()], vec![])
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a valid regex"), "{err}");

        // Hub::check_dial resolves and enforces in one step.
        let hub = Hub::new(
            loaded_config(),
            DialPolicy::build(vec!["^00".into()], vec![]).unwrap(),
            std::net::IpAddr::from([127, 0, 0, 1]),
        );
        assert_eq!(
            hub.check_dial("1002", "pbx.example.com").unwrap(),
            "sip:1002@pbx.example.com"
        );
        assert!(hub.check_dial("0044164123", "pbx.example.com").is_err());
    }

    #[test]
    fn hub_carries_custom_header_templates_into_the_slots() {
        let mut cfg = loaded_config();
        cfg.agents[0].custom_headers = vec![
            ("X-Static".into(), "fixed".into()),
            ("X-Session-Tag".into(), "session-${uuid}".into()),
        ];
        let hub = Hub::new(
            cfg,
            DialPolicy::default(),
            std::net::IpAddr::from([127, 0, 0, 1]),
        );
        let templates = &hub.slots[0].custom_headers;
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[1].1, "session-${uuid}");
    }

    #[test]
    fn closes_last_call_only_after_the_final_call_ended() {
        let mut s = AgentState::default();
        reduce(
            &mut s,
            &AppEvent::CallOutgoing {
                call_id: "c1".into(),
                number: "1002".into(),
            },
        );
        reduce(
            &mut s,
            &AppEvent::CallOutgoing {
                call_id: "c2".into(),
                number: "1003".into(),
            },
        );
        let closed = |id: &str| AppEvent::CallClosed {
            call_id: id.into(),
            reason: "ok".into(),
            error: false,
        };

        // First of two calls closes: another call remains — must NOT reset.
        reduce(&mut s, &closed("c1"));
        assert!(!closes_last_call(&closed("c1"), &s));

        // Last remaining call closes: reset.
        reduce(&mut s, &closed("c2"));
        assert!(closes_last_call(&closed("c2"), &s));

        // Non-close events never reset.
        assert!(!closes_last_call(
            &AppEvent::CallEstablished {
                call_id: "c3".into()
            },
            &s
        ));
    }

    #[test]
    fn get_rejects_unknown_agents_with_known_names() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let hub = Hub::new(
            loaded_config(),
            DialPolicy::default(),
            std::net::IpAddr::from([127, 0, 0, 1]),
        );
        let err = match rt.block_on(hub.get("mallory")) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("unknown agent must fail"),
        };
        assert!(err.contains("unknown agent `mallory`"), "{err}");
        assert!(err.contains("alice, bob"), "{err}");
    }
}
