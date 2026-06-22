//! The language-neutral scenario context: the live sessions, the reporter, the
//! default timeout and the assertion-reporting state. Script verbs operate on a
//! `Ctx` by agent name; a language adapter (e.g. `script::rhai`) holds an
//! `Arc<Ctx>` and exposes thin handles that call these methods.

use super::mock_server::MockServerInner;
use crate::runtime::agent_options;
use crate::runtime::report::{Event, Reporter};
use crate::runtime::session::AgentSession;
use crate::runtime::state::{CallPhase, received_header_value};
use ringo_core::baresip::Account;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::runtime::Handle;

/// A call phase, exposed by language adapters as `State::Idle`/`Ringing`/
/// `Established`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallState {
    Idle,
    Ringing,
    Established,
}

impl std::fmt::Display for CallState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CallState::Idle => "Idle",
            CallState::Ringing => "Ringing",
            CallState::Established => "Established",
        })
    }
}

/// A snapshot of an agent's observable state, for `info()` / `to_json()`.
pub struct AgentInfo {
    pub name: String,
    pub aor: String,
    pub registered: bool,
    pub state: CallState,
    /// Last closed call's reason, if any.
    pub reason: Option<String>,
    /// SIP status code parsed from `reason`, if it is a SIP response.
    pub status_code: Option<u16>,
    /// Current call's remote party `(uri, display_name)`, if there is a call
    /// (the caller for an incoming call).
    pub peer: Option<(String, Option<String>)>,
    /// Number of current calls (any phase).
    pub calls: usize,
}

/// A stashed assertion during `await_until` polling: `(desc, expect, actual, ok)`.
type StashedAssertion = (Option<String>, String, String, bool);

thread_local! {
    /// The label of the getter most recently read on this thread, so the next
    /// `assert(...)` can auto-label itself (`assert(caller.state)` logs as
    /// `Caller state: …` with no `describe`). Thread-local so `parallel` tasks
    /// can't cross-label each other.
    static PENDING_LABEL: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };

    /// While `await_until` polls, assertions on *this thread* don't emit (they'd
    /// spam); the last one is stashed and emitted once when it settles. Both are
    /// thread-local so concurrent `parallel` tasks don't corrupt each other's
    /// polling state.
    static ASSERT_SILENT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LAST_ASSERT: std::cell::RefCell<Option<StashedAssertion>> =
        const { std::cell::RefCell::new(None) };
}

/// Record the label of the getter just read (the auto-label for the next assert),
/// e.g. `"Caller state"` or `"HTTP status"`.
pub fn mark_pending_label(label: impl Into<String>) {
    PENDING_LABEL.with(|p| *p.borrow_mut() = Some(label.into()));
}

/// Take (and clear) the pending auto-label for the current thread, if any.
pub fn take_pending_label() -> Option<String> {
    PENDING_LABEL.with(|p| p.borrow_mut().take())
}

/// Shared, language-neutral host context. Holds the real sessions (so teardown is
/// central and script handles stay cheap), the reporter, the runtime handle for
/// the async bridge, and the assertion-reporting state.
pub struct Ctx {
    pub rt: Handle,
    pub reporter: Mutex<Box<dyn Reporter + Send>>,
    pub sessions: Mutex<HashMap<String, AgentSession>>,
    /// Mock HTTP servers started this scenario, shut down at `reset_sessions` so a
    /// port can't leak across scenarios (per-scenario isolation, like sessions).
    mock_servers: Mutex<Vec<Arc<MockServerInner>>>,
    /// Default `await_until` timeout in ms; settable via `default_timeout(…)`.
    default_timeout_ms: AtomicU64,
    /// Disable TLS cert verification for `http(...)` (the `--insecure-http` escape hatch).
    http_insecure: AtomicBool,
}

impl Ctx {
    pub fn new(rt: Handle, reporter: Box<dyn Reporter + Send>, default_timeout: Duration) -> Self {
        Self {
            rt,
            reporter: Mutex::new(reporter),
            sessions: Mutex::new(HashMap::new()),
            mock_servers: Mutex::new(Vec::new()),
            default_timeout_ms: AtomicU64::new(default_timeout.as_millis() as u64),
            http_insecure: AtomicBool::new(false),
        }
    }

    /// Disable TLS certificate verification for `http(...)`. DANGER — only for
    /// testing against internal services with a private CA / self-signed cert.
    pub fn set_http_insecure(&self, on: bool) {
        self.http_insecure.store(on, Ordering::Relaxed);
    }
    /// Whether `http(...)` should skip TLS certificate verification.
    pub fn http_insecure(&self) -> bool {
        self.http_insecure.load(Ordering::Relaxed)
    }

    pub fn emit(&self, event: &Event) {
        self.reporter
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .emit(event);
    }

    fn with_session<R>(&self, name: &str, f: impl FnOnce(&AgentSession) -> R) -> Result<R, String> {
        let map = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let session = map.get(name).ok_or_else(|| {
            format!("agent `{name}` is not connected — create it with `agent(\"{name}\", …)` first")
        })?;
        Ok(f(session))
    }

    fn act(&self, name: &str, kind: &'static str, detail: Option<&str>) {
        self.emit(&Event::Action {
            agent: name,
            kind,
            detail,
        });
    }

    // ── agent lifecycle ──
    /// Connect a headless baresip agent, register custom headers, store it.
    pub fn connect_agent(
        &self,
        name: &str,
        account: Account,
        headers: &[(String, String)],
    ) -> Result<(), String> {
        let aor = format!("sip:{}@{}", account.username, account.domain);
        let session = self
            .rt
            .block_on(AgentSession::connect(name, account, &agent_options()))
            .map_err(|e| format!("agent `{name}`: connect failed: {e}"))?;
        self.emit(&Event::AgentStarted { name, aor: &aor });
        for (key, value) in headers {
            session.add_header(key, value);
            self.emit(&Event::Action {
                agent: name,
                kind: "header",
                detail: Some(&format!("{key}: {value}")),
            });
        }
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_string(), session);
        Ok(())
    }

    /// Track a mock server so it is shut down at the next `reset_sessions`.
    pub fn register_mock(&self, server: Arc<MockServerInner>) {
        self.mock_servers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(server);
    }

    // ── getters ──
    // Each getter tags the thread's pending auto-label so a following
    // `assert(<agent>.<getter>)` is labeled with the agent — no `describe` needed.
    pub fn registered(&self, name: &str) -> Result<bool, String> {
        mark_pending_label(format!("{name} registered"));
        self.with_session(name, |s| s.state().borrow().registered)
    }
    pub fn call_state(&self, name: &str) -> Result<CallState, String> {
        mark_pending_label(format!("{name} state"));
        self.with_session(name, |s| {
            let rx = s.state();
            let st = rx.borrow();
            if st.calls.iter().any(|c| c.phase == CallPhase::Established) {
                CallState::Established
            } else if st.calls.iter().any(|c| c.phase == CallPhase::Ringing) {
                CallState::Ringing
            } else {
                CallState::Idle
            }
        })
    }
    /// Last closed call's reason, or `None`.
    pub fn reason(&self, name: &str) -> Result<Option<String>, String> {
        mark_pending_label(format!("{name} reason"));
        self.with_session(name, |s| s.state().borrow().last_call_reason.clone())
    }
    /// SIP status code parsed from the last closed call's reason, or `None`.
    pub fn status_code(&self, name: &str) -> Result<Option<u16>, String> {
        mark_pending_label(format!("{name} status code"));
        self.with_session(name, |s| {
            s.state()
                .borrow()
                .last_call_reason
                .as_deref()
                .and_then(sip_status_code)
        })
    }
    /// Value of a header on a received INVITE, or `None`.
    pub fn header(&self, name: &str, header: &str) -> Result<Option<String>, String> {
        mark_pending_label(format!("{name} header {header}"));
        self.with_session(name, |s| received_header_value(&s.state().borrow(), header))
    }
    /// A one-shot snapshot of the agent's observable state (for `info()`/`to_json()`).
    /// Reads the session once and marks no pending label (so it can't mis-label a
    /// following assertion).
    pub fn info(&self, name: &str) -> Result<AgentInfo, String> {
        self.with_session(name, |s| {
            let rx = s.state();
            let st = rx.borrow();
            let state = if st.calls.iter().any(|c| c.phase == CallPhase::Established) {
                CallState::Established
            } else if st.calls.iter().any(|c| c.phase == CallPhase::Ringing) {
                CallState::Ringing
            } else {
                CallState::Idle
            };
            AgentInfo {
                name: name.to_string(),
                aor: s.aor.clone(),
                registered: st.registered,
                state,
                reason: st.last_call_reason.clone(),
                status_code: st.last_call_reason.as_deref().and_then(sip_status_code),
                peer: st.peer(),
                calls: st.calls.len(),
            }
        })
    }
    /// The current call's remote party `(uri, display_name)`, or `None` if there's
    /// no call. No pending label is marked here — the script-side `Peer` field
    /// accessor labels it (so `caller.peer.number` → "Caller peer number").
    pub fn peer(&self, name: &str) -> Result<Option<(String, Option<String>)>, String> {
        self.with_session(name, |s| s.state().borrow().peer())
    }
    /// All received INVITE headers `(name, value)` the agent has seen, in order
    /// (duplicates preserved). Backs `headers()`.
    pub fn headers(&self, name: &str) -> Result<Vec<(String, String)>, String> {
        self.with_session(name, |s| s.state().borrow().received_headers_flat())
    }

    // ── call control ──
    pub fn register(&self, name: &str) -> Result<(), String> {
        self.with_session(name, AgentSession::register)?;
        self.act(name, "register", None);
        Ok(())
    }
    pub fn accept(&self, name: &str) -> Result<(), String> {
        self.with_session(name, AgentSession::accept)?;
        self.act(name, "accept", None);
        Ok(())
    }
    pub fn hangup(&self, name: &str) -> Result<(), String> {
        self.with_session(name, AgentSession::hangup)?;
        self.act(name, "hangup", None);
        Ok(())
    }
    pub fn hold(&self, name: &str) -> Result<(), String> {
        self.with_session(name, AgentSession::hold)?;
        self.act(name, "hold", None);
        Ok(())
    }
    pub fn resume(&self, name: &str) -> Result<(), String> {
        self.with_session(name, AgentSession::resume)?;
        self.act(name, "resume", None);
        Ok(())
    }
    pub fn mute(&self, name: &str) -> Result<(), String> {
        self.with_session(name, AgentSession::mute)?;
        self.act(name, "mute", None);
        Ok(())
    }
    /// Send DTMF digits. `gap` is the pause inserted *between* digits (not before
    /// the first or after the last); `Duration::ZERO` sends them back-to-back. The
    /// session lock is taken per digit, so the gap doesn't block other access.
    pub fn dtmf(&self, name: &str, digits: &str, gap: Duration) -> Result<(), String> {
        let digits: Vec<char> = digits.chars().filter(|c| !c.is_whitespace()).collect();
        for (i, c) in digits.iter().enumerate() {
            if i > 0 && !gap.is_zero() {
                std::thread::sleep(gap);
            }
            self.with_session(name, |s| s.send_dtmf(*c))?;
        }
        let detail: String = digits.iter().collect();
        self.act(name, "dtmf", Some(&detail));
        Ok(())
    }

    fn do_dial(&self, name: &str, uri: &str) -> Result<(), String> {
        self.with_session(name, |s| s.dial(uri))?;
        self.act(name, "dial", Some(uri));
        Ok(())
    }
    /// `a.dial(b)` — dial another agent at its AOR.
    pub fn dial_agent(&self, name: &str, target: &str) -> Result<(), String> {
        let uri = self.with_session(target, |s| s.aor.clone())?;
        self.do_dial(name, &uri)
    }
    /// `a.dial("sip:…" | "4915…")` — a literal URI, or a bare number/extension
    /// dialed in the agent's own domain.
    pub fn dial_uri(&self, name: &str, target: &str) -> Result<(), String> {
        let uri = self.resolve_uri(name, target)?;
        self.do_dial(name, &uri)
    }

    /// A literal SIP URI as-is, or a bare number/extension turned into a URI in
    /// `name`'s own domain (shared by `dial`/`transfer`).
    fn resolve_uri(&self, name: &str, target: &str) -> Result<String, String> {
        if target.starts_with("sip:") || target.contains('@') {
            Ok(target.to_string())
        } else {
            let domain = self.with_session(name, |s| s.domain().to_string())?;
            Ok(format!("sip:{target}@{domain}"))
        }
    }

    // ── transfer (REFER) ──
    /// Blind-transfer `name`'s call to another agent's AOR.
    pub fn transfer_agent(&self, name: &str, target: &str) -> Result<(), String> {
        let uri = self.with_session(target, |s| s.aor.clone())?;
        self.with_session(name, |s| s.transfer(&uri))?;
        self.act(name, "transfer", Some(&uri));
        Ok(())
    }
    /// Blind-transfer `name`'s call to a literal URI or bare number/extension.
    pub fn transfer_uri(&self, name: &str, target: &str) -> Result<(), String> {
        let uri = self.resolve_uri(name, target)?;
        self.with_session(name, |s| s.transfer(&uri))?;
        self.act(name, "transfer", Some(&uri));
        Ok(())
    }
    /// Start an attended transfer: place a consultation call to another agent.
    pub fn attended_transfer_agent(&self, name: &str, target: &str) -> Result<(), String> {
        let uri = self.with_session(target, |s| s.aor.clone())?;
        self.with_session(name, |s| s.attended_transfer_start(&uri))?;
        self.act(name, "attended-transfer", Some(&uri));
        Ok(())
    }
    /// Start an attended transfer to a literal URI or bare number/extension.
    pub fn attended_transfer_uri(&self, name: &str, target: &str) -> Result<(), String> {
        let uri = self.resolve_uri(name, target)?;
        self.with_session(name, |s| s.attended_transfer_start(&uri))?;
        self.act(name, "attended-transfer", Some(&uri));
        Ok(())
    }
    /// Complete the pending attended transfer (REFER with Replaces).
    pub fn complete_transfer(&self, name: &str) -> Result<(), String> {
        self.with_session(name, |s| s.attended_transfer_exec())?;
        self.act(name, "complete-transfer", None);
        Ok(())
    }
    /// Abort the pending attended transfer.
    pub fn abort_transfer(&self, name: &str) -> Result<(), String> {
        self.with_session(name, |s| s.attended_transfer_abort())?;
        self.act(name, "abort-transfer", None);
        Ok(())
    }

    // ── audio support (used by the audio verbs) ──
    pub fn set_audio_source(&self, name: &str, spec: &str) -> Result<(), String> {
        self.with_session(name, |s| s.set_audio_source(spec))
    }
    pub fn recording_dir(&self, name: &str) -> Result<std::path::PathBuf, String> {
        self.with_session(name, |s| s.recording_dir().to_path_buf())
    }
    pub fn emit_action(&self, name: &str, kind: &'static str, detail: Option<&str>) {
        self.act(name, kind, detail);
    }

    // ── assertions ──
    /// Record an assertion result. While silent (inside `await_until`), it's only
    /// stashed; otherwise it's emitted right away. Returns `ok`.
    pub fn report_assertion(
        &self,
        desc: Option<String>,
        expect: String,
        actual: String,
        ok: bool,
    ) -> bool {
        if ASSERT_SILENT.with(|s| s.get()) {
            LAST_ASSERT.with(|l| *l.borrow_mut() = Some((desc, expect, actual, ok)));
        } else {
            self.emit(&Event::Assertion {
                label: desc.as_deref(),
                expect,
                ok,
                actual: Some(actual),
            });
        }
        // Drop any auto-label left by an expected-side getter (e.g. the `y` in
        // `assert(a.state).equals(b.state)`) so it can't leak to the next assert.
        take_pending_label();
        ok
    }
    pub fn set_assert_silent(&self, silent: bool) {
        ASSERT_SILENT.with(|s| s.set(silent));
    }
    /// Emit the assertion stashed during `await_until` polling (its final state).
    pub fn emit_last_assert(&self) {
        if let Some((desc, expect, actual, ok)) = LAST_ASSERT.with(|l| l.borrow_mut().take()) {
            self.emit(&Event::Assertion {
                label: desc.as_deref(),
                expect,
                ok,
                actual: Some(actual),
            });
        }
    }

    /// Hang up and drop all sessions (per-scenario isolation / final teardown),
    /// letting baresip flush BYEs. Poison-tolerant so cleanup can't be blocked.
    pub fn reset_sessions(&self) {
        // Signal any mock servers to stop, then collect their task handles so we can
        // await actual socket release (so the next scenario can rebind an explicit
        // port without an "address in use" race).
        let servers: Vec<Arc<MockServerInner>> = self
            .mock_servers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        let mut server_tasks = Vec::with_capacity(servers.len());
        for s in &servers {
            s.shutdown();
            if let Some(task) = s.take_task() {
                server_tasks.push(task);
            }
        }

        let sessions: Vec<AgentSession> = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain()
            .map(|(_, s)| s)
            .collect();
        for s in &sessions {
            s.hangup_all();
        }
        self.rt.block_on(async {
            // Await server shutdown so ports are freed, then let baresip flush BYEs.
            for task in server_tasks {
                let _ = task.await;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        });
        drop(sessions);
        drop(servers);
    }

    pub fn default_timeout(&self) -> Duration {
        Duration::from_millis(self.default_timeout_ms.load(Ordering::Relaxed))
    }
    pub fn set_default_timeout(&self, d: Duration) {
        self.default_timeout_ms
            .store(d.as_millis() as u64, Ordering::Relaxed);
    }
}

/// The leading SIP status code in a `CALL_CLOSED` reason (`"603 Decline"` → 603),
/// or `None` if the reason doesn't start with a SIP status code (100–699).
pub fn sip_status_code(reason: &str) -> Option<u16> {
    let code = reason.split_whitespace().next()?.parse::<u16>().ok()?;
    (100..=699).contains(&code).then_some(code)
}

/// Best-effort user-part ("number") of a SIP/tel URI: drop the scheme, take up to
/// `@`, then strip any `;params`. `sip:492098@ex.com;transport=tls` → `492098`.
pub fn sip_user_part(uri: &str) -> String {
    let after_scheme = uri.split_once(':').map_or(uri, |(_, rest)| rest);
    let user = after_scheme.split('@').next().unwrap_or(after_scheme);
    user.split(';').next().unwrap_or(user).to_string()
}

#[cfg(test)]
mod tests {
    use super::{mark_pending_label, sip_status_code, take_pending_label};

    #[test]
    fn sip_status_code_parsed_from_reason() {
        assert_eq!(sip_status_code("603 Decline"), Some(603));
        assert_eq!(sip_status_code("486 Busy Here"), Some(486));
        assert_eq!(sip_status_code("200 OK"), Some(200));
        assert_eq!(sip_status_code("Connection reset by peer"), None);
        assert_eq!(sip_status_code(""), None);
        assert_eq!(sip_status_code("999 Bogus"), None); // out of 100..=699
    }

    #[test]
    fn pending_label_is_take_once() {
        assert_eq!(take_pending_label(), None); // nothing pending initially
        mark_pending_label("Caller state");
        assert_eq!(take_pending_label().as_deref(), Some("Caller state"));
        assert_eq!(take_pending_label(), None); // consumed — no leak to next assert
        // latest getter wins
        mark_pending_label("Caller state");
        mark_pending_label("Callee registered");
        assert_eq!(take_pending_label().as_deref(), Some("Callee registered"));
    }
}
