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

use crate::config::LoadedConfig;
use crate::state::{AgentState, reduce};
use anyhow::{Context, Result};
use regex::Regex;
use ringo_agent::{AgentConfig, ProcessClient, SavedAudio};
use ringo_core::account::{Account, BackendOptions};
use ringo_core::event::AppEvent;
use ringo_profile::header::{HeaderContext, HeaderTemplate};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as AsyncMutex, broadcast};

/// Capacity of the per-agent event broadcast channel. Events that nobody is
/// waiting for are dropped (the state fold still sees them); `wait_event`
/// callers that fall behind get a lag notification, not stale data.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// How long `dial_and_wait` waits for the `call_outgoing` confirmation before
/// concluding the worker rejected the dial locally (some targets produce no
/// call at all — no event, nothing to wait for).
const DIAL_CONFIRM_TIMEOUT: Duration = Duration::from_secs(10);

/// How many recent events each agent keeps in its log (the `after_id` cursor
/// window). Bounded by design: the log makes event loss *explicit*
/// (`truncated`), not impossible.
pub(crate) const EVENT_LOG_CAPACITY: usize = 256;
/// Upper bound on events returned by one `agent_events` call (page with
/// `after_id` = the last returned id).
pub(crate) const EVENT_PAGE_LIMIT: usize = 100;

/// A per-agent, bounded event log with monotonically increasing ids — the
/// `after_id` cursors of `wait_event`/`agent_events` read it oldest-first.
/// Lives in the agent's [`Slot`], NOT in the worker-backed `Agent`: a cursor
/// must survive worker respawns (fresh ids would hide behind a stale cursor).
pub(crate) struct EventLog {
    entries: Mutex<VecDeque<(u64, AppEvent)>>,
    next_id: std::sync::atomic::AtomicU64,
    notify: tokio::sync::Notify,
}

impl EventLog {
    /// Test constructor (the real one is wired by `Hub::new`).
    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self::new()
    }

    fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            next_id: std::sync::atomic::AtomicU64::new(1),
            notify: tokio::sync::Notify::new(),
        }
    }

    /// Append an event, assign its id, trim to capacity, wake the cursor
    /// waiters. Returns the id.
    pub(crate) fn append(&self, event: AppEvent) -> u64 {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.push_back((id, event));
        while entries.len() > EVENT_LOG_CAPACITY {
            entries.pop_front();
        }
        drop(entries);
        self.notify.notify_waiters();
        id
    }

    /// The oldest matching entry after `after_id`, oldest first.
    pub(crate) fn first_after(
        &self,
        after_id: u64,
        pred: impl Fn(&AppEvent) -> bool,
    ) -> Option<(u64, AppEvent)> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|(id, e)| *id > after_id && pred(e))
            .cloned()
    }

    /// Up to [`EVENT_PAGE_LIMIT`] entries after `after_id` (oldest first),
    /// plus the log bounds and whether `after_id` fell off the ring.
    pub(crate) fn page_after(&self, after_id: u64) -> (Vec<(u64, AppEvent)>, u64, u64, bool) {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let oldest = entries.front().map(|(id, _)| *id).unwrap_or(0);
        let newest = entries.back().map(|(id, _)| *id).unwrap_or(0).max(
            // An empty ring still reports the last assigned id so a client
            // holding it skips nothing once events flow again.
            self.next_id
                .load(std::sync::atomic::Ordering::Relaxed)
                .saturating_sub(1),
        );
        let truncated = after_id.saturating_add(1) < oldest;
        let events: Vec<(u64, AppEvent)> = entries
            .iter()
            .filter(|(id, _)| *id > after_id)
            .take(EVENT_PAGE_LIMIT)
            .cloned()
            .collect();
        (events, oldest, newest, truncated)
    }
}

/// How often the bridge polls the worker for newly seen inbound INVITE headers
/// (same cadence as ringo-flow's trace poll).
const HEADER_POLL_INTERVAL: Duration = Duration::from_millis(150);

/// The outcome of a `dial` with `wait_established`.
#[derive(Debug, Clone)]
pub enum DialOutcome {
    /// The call is up.
    Established {
        /// The established call's SIP Call-ID.
        call_id: String,
    },
    /// The call ended before it was established (busy, declined, …).
    Failed {
        /// The failed call's SIP Call-ID.
        call_id: String,
        /// Why it ended, as the backend reports it.
        reason: String,
    },
    /// The wait budget expired with the call still ringing — the call itself
    /// continues; the caller decides (hangup or keep waiting).
    StillRinging {
        /// The still-ringing call's SIP Call-ID.
        call_id: String,
    },
    /// The worker produced no call within [`DIAL_CONFIRM_TIMEOUT`] — the dial
    /// was likely rejected locally.
    NoCall,
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
}

/// One configured agent and its (optional) running instance. The async mutex
/// is the single-flight spawn lock for this agent: the first tool call holds
/// it while spawning; concurrent calls for the same agent wait and then see
/// the already-running instance.
struct Slot {
    name: String,
    /// The ringo-phone profile this agent was built from, if any.
    profile: Option<String>,
    aor: String,
    account: Account,
    options: BackendOptions,
    custom_headers: Vec<(String, String)>,
    /// The agent's event log (id-cursored) — lives here, not in `Agent`, so
    /// cursors survive worker respawns.
    log: Arc<EventLog>,
    agent: AsyncMutex<Option<Arc<Agent>>>,
}

impl Hub {
    /// Build the hub from a (already validated) config and the runtime CLI
    /// knobs (the config is agents-only). Spawns nothing.
    pub fn new(config: LoadedConfig, dial: DialPolicy) -> Self {
        let slots = config
            .agents
            .into_iter()
            .map(|def: crate::config::AgentDef| {
                let aor = format!("sip:{}@{}", def.account.username, def.account.domain);
                Slot {
                    name: def.name,
                    profile: def.profile,
                    aor,
                    options: config.backend.clone(),
                    custom_headers: def.custom_headers,
                    log: Arc::new(EventLog::new()),
                    agent: AsyncMutex::new(None),
                    account: def.account,
                }
            })
            .collect();
        Self { slots, dial }
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
                    profile: s.profile.clone(),
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
                slot.profile.clone(),
                slot.account.clone(),
                slot.options.clone(),
                &slot.custom_headers,
                Arc::clone(&slot.log),
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
        // grace) — off the async threads.
        tokio::task::spawn_blocking(move || drop(agent));
        Ok(true)
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
}

/// One configured agent as seen by `list_agents`: config facts plus, if the
/// worker is running, its reduced state.
pub struct AgentOverview {
    /// Config label (tool-facing name).
    pub name: String,
    /// The ringo-phone profile this agent was built from, if any.
    pub profile: Option<String>,
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
    /// The ringo-phone profile this agent was built from, if any.
    pub profile: Option<String>,
    /// `sip:username@domain` of the registered AOR.
    pub aor: String,
    /// The agent's SIP domain, used to resolve bare dial targets.
    pub domain: String,
    /// Recording bookkeeping, shared with the event bridge.
    recording: Arc<RecordingState>,
    client: Arc<ProcessClient>,
    state: Arc<Mutex<AgentState>>,
    events: broadcast::Sender<(u64, AppEvent)>,
    /// The agent's event log (owned by the hub `Slot`; ids survive respawns).
    log: Arc<EventLog>,
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
        profile: Option<String>,
        account: Account,
        options: BackendOptions,
        custom_headers: &[(String, String)],
        log: Arc<EventLog>,
    ) -> Result<Self> {
        let custom_headers: Vec<(String, HeaderTemplate)> = custom_headers
            .iter()
            .map(|(k, v)| (k.clone(), HeaderTemplate::new(v)))
            .collect();
        let aor = format!("sip:{}@{}", account.username, account.domain);
        let domain = account.domain.clone();
        let record_default = options.record_audio;
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

        let (events_tx, _) = broadcast::channel::<(u64, AppEvent)>(EVENT_CHANNEL_CAPACITY);
        let state = Arc::new(Mutex::new(AgentState::default()));
        // `record_default` agents are armed at startup (the worker's
        // BackendOptions carries it); see `RecordingState` for the per-call policy.
        let recording = Arc::new(RecordingState::new(record_default));

        // Bridge the worker's blocking std channel into the async world: reduce
        // into the shared state, fan out to any wait_event subscribers, poll for
        // inbound INVITE headers between events, reset a stale `play` spec
        // when the agent's last call ends, and keep the recording flags in step.
        let bridge_state = Arc::clone(&state);
        let bridge_tx = events_tx.clone();
        let bridge_client = Arc::clone(&client);
        let bridge_log = Arc::clone(&log);
        let bridge_recording = Arc::clone(&recording);
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
                        // Log first (assigns the id the cursors see), then fan
                        // out — a cursor waiter waking on the notify finds the
                        // entry already in the log.
                        let id = bridge_log.append(event.clone());
                        let _ = bridge_tx.send((id, event.clone()));
                        let mut g = bridge_state.lock().unwrap_or_else(|e| e.into_inner());
                        let starts_call = matches!(
                            &event,
                            AppEvent::CallIncoming { .. } | AppEvent::CallOutgoing { .. }
                        );
                        let was_idle = g.calls.is_empty();
                        reduce(&mut g, &event);
                        // `play` is call-scoped for MCP agents: when the agent's LAST
                        // call ended, stop transmitting the stale spec so the next
                        // call doesn't replay it (the underlying ausrc is per-UA and
                        // would otherwise persist, as ringo-flow scenarios want).
                        let last_call_ended = closes_last_call(&event, &g);
                        if last_call_ended {
                            bridge_client.set_audio_source("silence");
                        }
                        match bridge_recording.on_event(
                            starts_call && was_idle,
                            matches!(event, AppEvent::CallOutgoing { .. }),
                            last_call_ended,
                        ) {
                            RecordingAction::Arm => bridge_client.set_recording(true),
                            RecordingAction::Disarm => bridge_client.set_recording(false),
                            RecordingAction::None => {}
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
            profile,
            aor,
            domain,
            recording,
            client,
            state,
            events: events_tx,
            log,
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

    /// Wait for the agent's next event, or `None` on timeout.
    ///
    /// **Without** `after_id`: subscribes at call time — events that happened
    /// before are not replayed (poll `agent_status` for state instead).
    ///
    /// **With** `after_id` (a cursor from a previous reply): reads the agent's
    /// event LOG oldest-first, so events that fired between the caller's
    /// actions are not lost — the roundtrip gap that swallows a
    /// `call_established` between `accept` and `wait_event` closes. Returns
    /// the matched event with its id (the caller's next cursor).
    ///
    /// With `names`, events that don't match one of the given names are
    /// skipped (still folded into the agent state, logged and visible to
    /// other waiters) — `None` means any event.
    pub async fn wait_event(
        &self,
        timeout: Duration,
        names: Option<&[&str]>,
        after_id: Option<u64>,
    ) -> Option<(u64, AppEvent)> {
        match after_id {
            None => {
                let mut rx = self.events.subscribe();
                tokio::time::timeout(timeout, async {
                    loop {
                        match rx.recv().await {
                            Ok((id, event)) => {
                                if names.is_none_or(|n| n.contains(&crate::event_name(&event))) {
                                    return Some((id, event));
                                }
                            }
                            // Fell behind the ring: skip the notice, next event
                            // follows.
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => return None,
                        }
                    }
                })
                .await
                .ok()
                .flatten()
            }
            Some(cursor) => {
                let matches =
                    |e: &AppEvent| names.is_none_or(|n| n.contains(&crate::event_name(e)));
                // Fast path: the log already holds a match — the very gap this
                // cursor mode exists for.
                if let Some(hit) = self.log.first_after(cursor, matches) {
                    return Some(hit);
                }
                // Armed-future pattern: each loop iteration arms the waiter,
                // THEN checks the log (an append between iterations is caught
                // by the check, one during the check by the notify).
                tokio::time::timeout(timeout, async {
                    loop {
                        let notified = std::pin::pin!(self.log.notify.notified());
                        if let Some(hit) = self.log.first_after(cursor, matches) {
                            return Some(hit);
                        }
                        notified.await;
                    }
                })
                .await
                .ok()
                .flatten()
            }
        }
    }

    /// The agent's event log bounds (oldest/newest id), for `agent_events`.
    pub(crate) fn event_log_page(&self, after_id: u64) -> (Vec<(u64, AppEvent)>, u64, u64, bool) {
        self.log.page_after(after_id)
    }

    /// Dial the (already policy-checked, resolved) target URI, re-rendering
    /// dynamic header templates first (fresh `${uuid}` per call).
    pub fn dial(&self, resolved: &str) {
        self.refresh_dynamic_headers();
        self.client.dial(resolved);
    }

    /// Dial and block for the outcome (`dial`'s `wait_established`): the
    /// subscription happens BEFORE the dial command is sent (same task →
    /// ordered), so the confirmation can't race the subscription. Events
    /// still reach every other consumer — the broadcast fan-out is shared,
    /// nothing is swallowed.
    ///
    /// Call attribution: the FIRST `call_outgoing` after the subscribe is
    /// ours (single-client heuristic; concurrent dials on the same agent are
    /// ambiguous, see the tool docs). `call_id`s of other calls are skipped.
    pub async fn dial_and_wait(&self, resolved: &str, timeout: Duration) -> DialOutcome {
        let mut rx = self.events.subscribe();
        self.dial(resolved);
        let deadline = Instant::now() + timeout;

        // Phase 1: our call id arrives with the outgoing event.
        let confirm_budget = DIAL_CONFIRM_TIMEOUT.min(timeout);
        let confirmed = tokio::time::timeout(confirm_budget, async {
            loop {
                match rx.recv().await {
                    Ok((_, AppEvent::CallOutgoing { call_id, .. })) => return Some(call_id),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    // Worker gone: nothing will ever confirm.
                    Err(broadcast::error::RecvError::Closed) => return None,
                    Ok(_) => {}
                }
            }
        })
        .await;
        let our_id = match confirmed {
            Ok(Some(id)) => id,
            _ => return DialOutcome::NoCall,
        };

        // Phase 2: established / closed for OUR call, or the overall budget
        // (the call keeps ringing past it — no auto-cancel by design).
        let remaining = deadline.saturating_duration_since(Instant::now());
        let other_id = our_id.clone();
        tokio::time::timeout(remaining, async {
            loop {
                match rx.recv().await {
                    Ok((_, AppEvent::CallEstablished { call_id })) if call_id == our_id => {
                        return DialOutcome::Established {
                            call_id: call_id.clone(),
                        };
                    }
                    Ok((
                        _,
                        AppEvent::CallClosed {
                            call_id, reason, ..
                        },
                    )) if call_id == our_id => {
                        return DialOutcome::Failed {
                            call_id: call_id.clone(),
                            reason,
                        };
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return DialOutcome::Failed {
                            call_id: other_id,
                            reason: "agent worker is gone".to_string(),
                        };
                    }
                    Ok(_) => {}
                }
            }
        })
        .await
        .unwrap_or(DialOutcome::StillRinging {
            call_id: our_id.clone(),
        })
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

    /// Arm or disarm full-call recording for the agent (see
    /// `record_start`/`record_stop`): arming starts a fresh recording, disarming
    /// keeps what was recorded until the next arm/call (save with
    /// [`Self::save_audio`]). A per-call recording is disarmed automatically
    /// when the agent's last call ends.
    pub fn set_recording(&self, on: bool) {
        // Flags before the command, so a close the bridge processes in between
        // already sees them.
        if on {
            self.recording.arm(false);
        } else {
            self.recording.disarm();
        }
        self.client.set_recording(on);
    }

    /// Arm a fresh recording for the call [`Self::dial`] is about to place
    /// (`dial` with `record: true`). Unlike [`Self::set_recording`], a close of
    /// the previous call that is still in flight won't disarm it.
    pub fn arm_for_next_call(&self) {
        self.recording.arm(true);
        self.client.set_recording(true);
    }

    /// Whether recording was ever armed for this agent (default, tool or
    /// param) — the gate for `save_audio`'s "recording is off" error.
    pub fn ever_recorded(&self) -> bool {
        self.recording.ever_armed()
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

    /// The worker's OS process id (names its SIP trace file).
    pub fn worker_pid(&self) -> u32 {
        self.client.pid()
    }

    /// Write the recorded sent+received audio to WAV files under `prefix`;
    /// returns what each file contains (path, side, duration, truncation).
    /// Empty when nothing was captured; gate with [`Self::ever_recorded`] to
    /// tell "recording is off" from "no audio yet".
    pub fn save_audio(&self, prefix: &str) -> Vec<SavedAudio> {
        self.client.save_audio_detailed(prefix)
    }
}

/// Resolve a dial/transfer target: full URIs pass through, `user@host` gets a
/// `sip:` prefix, and a bare number/extension becomes `sip:<target>@<domain>`.
/// Input is cleaned up first (see [`sanitize_target`]).
fn resolve_target(target: &str, domain: &str) -> String {
    let target = sanitize_target(target);
    if target.starts_with("sip:") || target.starts_with("sips:") {
        target
    } else if target.contains('@') {
        format!("sip:{target}")
    } else {
        format!("sip:{target}@{domain}")
    }
}

/// Clean up a dial target the way people (and LLMs) write it, mirroring
/// ringo-phone's `sanitize_dial_target`: trim, drop `tel:`/`callto:`,
/// lowercase a `sip:`/`sips:` scheme, and strip separators (spaces, `-`, `.`,
/// `/`, parentheses) from phone numbers. SIP user names and URIs keep their
/// characters.
fn sanitize_target(input: &str) -> String {
    let trimmed = input.trim();
    let (scheme, rest) = trimmed.split_once(':').unwrap_or(("", trimmed));
    let trimmed = match scheme.to_ascii_lowercase().as_str() {
        "tel" | "callto" => rest.trim(),
        s @ ("sip" | "sips") => return format!("{s}:{rest}"),
        _ => trimmed,
    };
    if trimmed.contains('@') || trimmed.chars().any(|c| c.is_ascii_alphabetic()) {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .filter(|c| !(c.is_whitespace() || matches!(c, '-' | '.' | '/' | '(' | ')')))
        .collect()
}

/// Whether this event ended the agent's last remaining call (post-reduce
/// state) — the trigger for resetting a stale `play` spec to silence.
fn closes_last_call(event: &AppEvent, state: &AgentState) -> bool {
    matches!(event, AppEvent::CallClosed { .. }) && state.calls.is_empty()
}

/// What the worker should be told about recording.
#[derive(Debug, PartialEq, Eq)]
enum RecordingAction {
    /// (Re-)arm full capture: clears the buffers — a fresh recording.
    Arm,
    /// Disarm; the buffers are kept for a later save.
    Disarm,
    None,
}

/// The agent's recording bookkeeping, shared by the tools (`record_start`,
/// `record: true`) and the event bridge, which arms/disarms per call.
///
/// Policy: with `record_default`, every call that starts from idle re-arms
/// (a fresh recording per call) and nothing is ever disarmed automatically;
/// without it, a recording armed by a tool is disarmed when the last call
/// ends, keeping its audio for `save_audio` until the next recording.
///
/// Every arm takes a fresh generation. A `dial` with `record: true` reserves
/// its generation for the call it is about to place, so a close of the
/// previous call that the bridge processes only afterwards doesn't disarm it.
#[derive(Debug)]
struct RecordingState {
    record_default: bool,
    /// Generation of the armed recording, 0 = not armed.
    current: AtomicU64,
    /// Generation reserved for the next dialed call, 0 = none.
    reserved: AtomicU64,
    next_generation: AtomicU64,
    /// Whether recording was ever armed (gates `save_audio`'s error).
    ever: AtomicBool,
}

impl RecordingState {
    fn new(record_default: bool) -> Self {
        Self {
            record_default,
            current: AtomicU64::new(u64::from(record_default)),
            reserved: AtomicU64::new(0),
            next_generation: AtomicU64::new(2),
            ever: AtomicBool::new(record_default),
        }
    }

    /// Arm a fresh recording (flags only; the caller tells the worker).
    /// `for_next_dial` reserves it for the call about to be dialed.
    fn arm(&self, for_next_dial: bool) {
        let generation = self.next_generation.fetch_add(1, Ordering::AcqRel);
        if for_next_dial {
            self.reserved.store(generation, Ordering::Release);
        }
        self.current.store(generation, Ordering::Release);
        self.ever.store(true, Ordering::Relaxed);
    }

    fn disarm(&self) {
        self.current.store(0, Ordering::Release);
        self.reserved.store(0, Ordering::Release);
    }

    fn ever_armed(&self) -> bool {
        self.ever.load(Ordering::Relaxed)
    }

    /// The bridge's decision for one event (after the state fold):
    /// `started_from_idle` = a call began while none was up, `dialed` = it is
    /// our outgoing call, `last_call_ended` = no call is left.
    fn on_event(
        &self,
        started_from_idle: bool,
        dialed: bool,
        last_call_ended: bool,
    ) -> RecordingAction {
        if dialed {
            // The dialed call claims a recording reserved for it.
            self.reserved.store(0, Ordering::Release);
        }
        if started_from_idle && self.record_default {
            self.arm(false);
            return RecordingAction::Arm;
        }
        if last_call_ended && !self.record_default {
            let current = self.current.load(Ordering::Acquire);
            let reserved = self.reserved.load(Ordering::Acquire);
            // Only the recording that belonged to the ended calls; a tool that
            // re-armed concurrently wins the compare-exchange.
            if current != 0
                && current != reserved
                && self
                    .current
                    .compare_exchange(current, 0, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                return RecordingAction::Disarm;
            }
        }
        RecordingAction::None
    }
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

    #[test]
    fn cleans_up_targets_the_way_people_and_llms_write_them() {
        let d = "pbx.example.com";
        // Separators in phone numbers.
        assert_eq!(
            resolve_target("+49 89 123456", d),
            "sip:+4989123456@pbx.example.com"
        );
        assert_eq!(
            resolve_target("(0123) 4567-890", d),
            "sip:01234567890@pbx.example.com"
        );
        assert_eq!(
            resolve_target("0123.4567/890", d),
            "sip:01234567890@pbx.example.com"
        );
        assert_eq!(resolve_target("  100  ", d), "sip:100@pbx.example.com");
        // Click-to-dial schemes.
        assert_eq!(
            resolve_target("tel:+49-30-1234567", d),
            "sip:+49301234567@pbx.example.com"
        );
        assert_eq!(
            resolve_target("callto:0123 456", d),
            "sip:0123456@pbx.example.com"
        );
        assert_eq!(resolve_target("TEL:100", d), "sip:100@pbx.example.com");
        // Scheme case is normalised; the rest of a URI is kept verbatim.
        assert_eq!(
            resolve_target("SIP:1002@Other.example.com", d),
            "sip:1002@Other.example.com"
        );
        assert_eq!(
            resolve_target("Sips:1002@other.example.com", d),
            "sips:1002@other.example.com"
        );
        // Dialable symbols and SIP user names survive.
        assert_eq!(resolve_target("*100#", d), "sip:*100#@pbx.example.com");
        assert_eq!(
            resolve_target("john.doe", d),
            "sip:john.doe@pbx.example.com"
        );
        assert_eq!(
            resolve_target("alice-bob", d),
            "sip:alice-bob@pbx.example.com"
        );
        assert_eq!(resolve_target("user@host", d), "sip:user@host");
    }

    #[test]
    fn dial_policy_sees_the_cleaned_up_target() {
        let hub = Hub::new(
            loaded_config(),
            DialPolicy::build(vec!["^00".into(), "^\\+?49900".into()], vec![]).unwrap(),
        );
        // Separators and schemes can't sneak a denied number past the policy.
        for target in [
            "00 41 1234",
            "tel:0041-1234",
            "+49 900 123",
            "TEL:+49900123",
        ] {
            assert!(
                hub.check_dial(target, "pbx.example.com").is_err(),
                "{target} must be denied"
            );
        }
    }

    fn loaded_config() -> LoadedConfig {
        LoadedConfig {
            agents: vec![
                crate::config::AgentDef {
                    name: "alice".into(),
                    profile: None,
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
                    profile: None,
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
        let hub = Hub::new(loaded_config(), DialPolicy::default());
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
        let hub = Hub::new(cfg, DialPolicy::default());
        let templates = &hub.slots[0].custom_headers;
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[1].1, "session-${uuid}");
    }

    #[test]
    fn default_recording_rearms_per_call_and_never_disarms() {
        use RecordingAction::*;
        let r = RecordingState::new(true);
        assert!(r.ever_armed());
        // Re-armed at every call start from idle (a fresh recording per call)…
        assert_eq!(r.on_event(true, true, false), Arm);
        // …not mid-call (a second call mixes into the running recording)…
        assert_eq!(r.on_event(false, false, false), None);
        // …and kept armed when the last call ends (no gap before the next).
        assert_eq!(r.on_event(false, false, true), None);
        assert_eq!(r.on_event(true, false, false), Arm);
    }

    #[test]
    fn per_call_recording_is_disarmed_when_the_last_call_ends() {
        use RecordingAction::*;
        let r = RecordingState::new(false);
        assert!(!r.ever_armed());
        // Nothing armed: call events change nothing.
        assert_eq!(r.on_event(true, true, false), None);
        assert_eq!(r.on_event(false, false, true), None);

        // record_start, then a call: armed across the call start…
        r.arm(false);
        assert!(r.ever_armed());
        assert_eq!(r.on_event(true, false, false), None);
        // …and disarmed once (buffers kept for a save) when it ends.
        assert_eq!(r.on_event(false, false, true), Disarm);
        assert_eq!(r.on_event(false, false, true), None);

        // record_stop clears it explicitly: nothing left to auto-disarm.
        r.arm(false);
        r.disarm();
        assert_eq!(r.on_event(false, false, true), None);
    }

    #[test]
    fn record_dial_survives_the_previous_calls_close_in_flight() {
        use RecordingAction::*;
        let r = RecordingState::new(false);
        // Call 1 recorded; it ends, but the bridge hasn't seen the close yet
        // when `dial` with `record: true` arms for call 2.
        r.arm(false);
        r.arm(true);
        // The late close must not disarm call 2's recording…
        assert_eq!(r.on_event(false, false, true), None);
        // …call 2 claims it…
        assert_eq!(r.on_event(true, true, false), None);
        // …and it is disarmed when call 2 itself ends.
        assert_eq!(r.on_event(false, false, true), Disarm);
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
        let hub = Hub::new(loaded_config(), DialPolicy::default());
        let err = match rt.block_on(hub.get("mallory")) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("unknown agent must fail"),
        };
        assert!(err.contains("unknown agent `mallory`"), "{err}");
        assert!(err.contains("alice, bob"), "{err}");
    }
}
