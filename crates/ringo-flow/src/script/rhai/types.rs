//! The Rhai-facing value types — `Agent`, `Assertion`, `HttpResponse` — each a
//! thin handle that converts Rhai values and delegates to the neutral
//! [`crate::engine`]. Their methods are wired into the engine in `bindings`.

use super::convert;
use crate::engine::assertion::Assertion as EngineAssertion;
use crate::engine::ctx::{CallState, Ctx, mark_pending_label};
use crate::engine::http::{HttpResponse as EngineHttp, json_path_value};
use crate::engine::mock_server::{MockRequest as EngineMockRequest, MockServerInner, PathMatcher};
use crate::engine::sip_user_part;
use rhai::{Dynamic, EvalAltResult};
use std::sync::Arc;

/// A cheap handle to an agent: its name plus the shared context.
#[derive(Clone)]
pub(super) struct Agent {
    pub(super) name: String,
    pub(super) ctx: Arc<Ctx>,
}

/// Result of a verb/getter: referencing an unconnected agent fails the scenario
/// (a clean Rhai runtime error) instead of panicking the whole run.
type Verb<T> = Result<T, Box<EvalAltResult>>;

impl Agent {
    pub(super) fn registered(&mut self) -> Verb<bool> {
        self.ctx.registered(&self.name).map_err(|e| e.into())
    }
    pub(super) fn call_state(&mut self) -> Verb<CallState> {
        self.ctx.call_state(&self.name).map_err(|e| e.into())
    }
    pub(super) fn reason(&mut self) -> Verb<Dynamic> {
        self.ctx
            .reason(&self.name)
            .map(convert::opt_to_dynamic)
            .map_err(|e| e.into())
    }
    pub(super) fn status_code(&mut self) -> Verb<Dynamic> {
        self.ctx
            .status_code(&self.name)
            .map(|c| match c {
                Some(code) => (code as i64).into(),
                None => Dynamic::UNIT,
            })
            .map_err(|e| e.into())
    }
    pub(super) fn header(&mut self, name: &str) -> Verb<Dynamic> {
        self.ctx
            .header(&self.name, name)
            .map(convert::opt_to_dynamic)
            .map_err(|e| e.into())
    }
    /// The current call's remote party (the caller for an incoming call). Always
    /// returns a handle; its `.uri`/`.number`/`.name` fields are `()` when there's
    /// no call, and each field auto-labels itself (`caller.peer.number` →
    /// "Caller peer number").
    pub(super) fn peer(&mut self) -> Verb<Peer> {
        let (uri, name) = match self
            .ctx
            .peer(&self.name)
            .map_err(|e| -> Box<EvalAltResult> { e.into() })?
        {
            Some((uri, name)) => (Some(uri), name),
            None => (None, None),
        };
        Ok(Peer {
            agent: self.name.clone(),
            uri,
            name,
        })
    }
    /// All received INVITE headers as a map (name → value).
    pub(super) fn headers(&mut self) -> Verb<Dynamic> {
        self.ctx
            .headers(&self.name)
            .map(convert::headers_to_map)
            .map_err(|e| e.into())
    }
    /// A map of the agent's current observable state (name, aor, registered,
    /// state, reason, status_code, peer, calls).
    pub(super) fn info(&mut self) -> Verb<Dynamic> {
        self.ctx
            .info(&self.name)
            .map(|i| convert::info_to_map(&i))
            .map_err(|e| e.into())
    }
    /// The same snapshot as a JSON string (for `log(...)`/debugging).
    // Rhai method receivers are `&mut self`; the `to_*` naming is the user-facing
    // Rhai convention (mirrors a map's `to_json`), so the lint doesn't apply.
    #[allow(clippy::wrong_self_convention)]
    pub(super) fn to_json(&mut self) -> Verb<String> {
        let i = self
            .ctx
            .info(&self.name)
            .map_err(|e| -> Box<EvalAltResult> { e.into() })?;
        let peer = i.peer.as_ref().map(|(uri, name)| {
            serde_json::json!({
                "uri": uri,
                "number": crate::engine::sip_user_part(uri),
                "name": name,
            })
        });
        let v = serde_json::json!({
            "name": i.name,
            "aor": i.aor,
            "registered": i.registered,
            "state": i.state.to_string(),
            "reason": i.reason,
            "status_code": i.status_code,
            "peer": peer,
            "calls": i.calls,
        });
        serde_json::to_string(&v).map_err(|e| e.to_string().into())
    }

    pub(super) fn register(&mut self) -> Verb<()> {
        self.ctx.register(&self.name).map_err(|e| e.into())
    }
    pub(super) fn accept(&mut self) -> Verb<()> {
        self.ctx.accept(&self.name).map_err(|e| e.into())
    }
    pub(super) fn hangup(&mut self) -> Verb<()> {
        self.ctx.hangup(&self.name).map_err(|e| e.into())
    }
    pub(super) fn hold(&mut self) -> Verb<()> {
        self.ctx.hold(&self.name).map_err(|e| e.into())
    }
    pub(super) fn resume(&mut self) -> Verb<()> {
        self.ctx.resume(&self.name).map_err(|e| e.into())
    }
    pub(super) fn mute(&mut self) -> Verb<()> {
        self.ctx.mute(&self.name).map_err(|e| e.into())
    }
    pub(super) fn dtmf(&mut self, digits: &str) -> Verb<()> {
        self.ctx
            .dtmf(&self.name, digits, std::time::Duration::ZERO)
            .map_err(|e| e.into())
    }
    pub(super) fn dtmf_spaced(&mut self, digits: &str, gap: &str) -> Verb<()> {
        let gap = crate::engine::duration::parse_duration(gap)?;
        self.ctx.dtmf(&self.name, digits, gap).map_err(|e| e.into())
    }
    pub(super) fn dial_agent(&mut self, target: Agent) -> Verb<()> {
        self.ctx
            .dial_agent(&self.name, &target.name)
            .map_err(|e| e.into())
    }
    pub(super) fn dial_uri(&mut self, target: &str) -> Verb<()> {
        self.ctx.dial_uri(&self.name, target).map_err(|e| e.into())
    }

    pub(super) fn transfer_agent(&mut self, target: Agent) -> Verb<()> {
        self.ctx
            .transfer_agent(&self.name, &target.name)
            .map_err(|e| e.into())
    }
    pub(super) fn transfer_uri(&mut self, target: &str) -> Verb<()> {
        self.ctx
            .transfer_uri(&self.name, target)
            .map_err(|e| e.into())
    }
    pub(super) fn attended_transfer_agent(&mut self, target: Agent) -> Verb<()> {
        self.ctx
            .attended_transfer_agent(&self.name, &target.name)
            .map_err(|e| e.into())
    }
    pub(super) fn attended_transfer_uri(&mut self, target: &str) -> Verb<()> {
        self.ctx
            .attended_transfer_uri(&self.name, target)
            .map_err(|e| e.into())
    }
    pub(super) fn complete_transfer(&mut self) -> Verb<()> {
        self.ctx.complete_transfer(&self.name).map_err(|e| e.into())
    }
    pub(super) fn abort_transfer(&mut self) -> Verb<()> {
        self.ctx.abort_transfer(&self.name).map_err(|e| e.into())
    }
}

/// The current call's remote party, returned by `agent.peer`. Its field getters
/// carry the agent name so each auto-labels the assertion (`caller.peer.number`
/// → "Caller peer number"); every field is `()` when there's no call.
#[derive(Clone)]
pub(super) struct Peer {
    agent: String,
    uri: Option<String>,
    name: Option<String>,
}

impl Peer {
    pub(super) fn uri(&mut self) -> Dynamic {
        mark_pending_label(format!("{} peer uri", self.agent));
        convert::opt_to_dynamic(self.uri.clone())
    }
    pub(super) fn number(&mut self) -> Dynamic {
        mark_pending_label(format!("{} peer number", self.agent));
        convert::opt_to_dynamic(self.uri.as_deref().map(sip_user_part))
    }
    pub(super) fn name(&mut self) -> Dynamic {
        mark_pending_label(format!("{} peer name", self.agent));
        convert::opt_to_dynamic(self.name.clone())
    }
    /// Printable form (`print(caller.peer)`): the URI, or empty when no call.
    pub(super) fn display(&mut self) -> String {
        self.uri.clone().unwrap_or_default()
    }
}

/// A fluent assertion handle wrapping the engine assertion; matchers chain by
/// returning the handle, and errors surface as Rhai runtime errors.
#[derive(Clone)]
pub(super) struct Assertion {
    inner: EngineAssertion,
}

/// Matchers return the `Assertion` again so they can be chained
/// (`assert(x).at_least(200).at_most(299)`); a failure short-circuits as an error.
type Check = Result<Assertion, Box<EvalAltResult>>;

impl Assertion {
    pub(super) fn new(ctx: Arc<Ctx>, actual: Dynamic) -> Self {
        Self {
            inner: EngineAssertion::new(ctx, convert::to_value(&actual)),
        }
    }
    /// Turn an engine matcher result into a chainable handle (or an error).
    fn chain(&self, r: Result<(), String>) -> Check {
        r.map(|_| self.clone()).map_err(|e| e.into())
    }

    pub(super) fn describe(&mut self, label: &str) -> Assertion {
        self.inner.describe(label);
        self.clone()
    }
    pub(super) fn value(&mut self) -> Dynamic {
        convert::to_dynamic(self.inner.value())
    }

    pub(super) fn equals(&mut self, expected: Dynamic) -> Check {
        self.chain(self.inner.equals(&convert::to_value(&expected)))
    }
    pub(super) fn not_equals(&mut self, expected: Dynamic) -> Check {
        self.chain(self.inner.not_equals(&convert::to_value(&expected)))
    }
    pub(super) fn is_true(&mut self) -> Check {
        self.chain(self.inner.is_true())
    }
    pub(super) fn is_false(&mut self) -> Check {
        self.chain(self.inner.is_false())
    }
    pub(super) fn is_present(&mut self) -> Check {
        self.chain(self.inner.is_present())
    }
    pub(super) fn is_empty(&mut self) -> Check {
        self.chain(self.inner.is_empty())
    }
    pub(super) fn is_not_empty(&mut self) -> Check {
        self.chain(self.inner.is_not_empty())
    }
    pub(super) fn is_absent(&mut self) -> Check {
        self.chain(self.inner.is_absent())
    }
    pub(super) fn contains(&mut self, needle: &str) -> Check {
        self.chain(self.inner.contains(needle))
    }
    pub(super) fn matches(&mut self, pattern: &str) -> Check {
        self.chain(self.inner.matches(pattern))
    }
    pub(super) fn greater_than(&mut self, n: i64) -> Check {
        self.chain(self.inner.greater_than(n))
    }
    pub(super) fn at_least(&mut self, n: i64) -> Check {
        self.chain(self.inner.at_least(n))
    }
    pub(super) fn less_than(&mut self, n: i64) -> Check {
        self.chain(self.inner.less_than(n))
    }
    pub(super) fn at_most(&mut self, n: i64) -> Check {
        self.chain(self.inner.at_most(n))
    }
}

/// An HTTP response handle wrapping the engine response.
#[derive(Clone)]
pub(super) struct HttpResponse {
    pub(super) inner: EngineHttp,
}

impl HttpResponse {
    pub(super) fn status(&mut self) -> i64 {
        self.inner.status()
    }
    pub(super) fn body(&mut self) -> String {
        self.inner.body()
    }
    pub(super) fn header(&mut self, name: &str) -> Dynamic {
        convert::opt_to_dynamic(self.inner.header(name))
    }
    /// `res.json("data.id")` — the value at a dotted path, as a native Rhai value
    /// (object→map, array, number, bool, `null`→`()`).
    pub(super) fn json(&mut self, path: &str) -> Result<Dynamic, Box<EvalAltResult>> {
        self.inner
            .json(path)
            .map(|v| convert::json_to_dynamic(&v))
            .map_err(|e| e.into())
    }
    /// `res.json()` — the whole body as a native Rhai value.
    pub(super) fn json_all(&mut self) -> Result<Dynamic, Box<EvalAltResult>> {
        self.inner
            .json("")
            .map(|v| convert::json_to_dynamic(&v))
            .map_err(|e| e.into())
    }
    pub(super) fn expect_status(&mut self, code: i64) -> Result<(), Box<EvalAltResult>> {
        self.inner.expect_status(code).map_err(|e| e.into())
    }
}

/// A handle to a running mock HTTP server. Cheap to clone (shares the server's
/// state via `Arc`); the server is shut down automatically at the end of the
/// scenario, so scripts rarely call `stop()`.
#[derive(Clone)]
pub(super) struct HttpMock {
    pub(super) inner: Arc<MockServerInner>,
}

impl HttpMock {
    pub(super) fn url(&mut self) -> String {
        self.inner.url()
    }
    pub(super) fn port(&mut self) -> i64 {
        self.inner.port() as i64
    }
    /// Number of requests whose path matches; pair with `await_until`, e.g.
    /// `await_until(|| assert(hooks.request_count("/voice")).equals(1))`.
    pub(super) fn request_count(&mut self, path: &str) -> i64 {
        self.count(&PathMatcher::Exact(path.to_string()))
    }
    pub(super) fn request_count_re(&mut self, pat: PathPattern) -> i64 {
        self.count(&pat.inner)
    }
    fn count(&self, m: &PathMatcher) -> i64 {
        mark_pending_label(format!("mock requests to {}", m.label()));
        self.inner.request_count(m)
    }
    /// The most recent request whose path matches, for inspection after `await_until`.
    pub(super) fn last_request(&mut self, path: &str) -> Result<MockRequest, Box<EvalAltResult>> {
        self.last(&PathMatcher::Exact(path.to_string()))
    }
    pub(super) fn last_request_re(
        &mut self,
        pat: PathPattern,
    ) -> Result<MockRequest, Box<EvalAltResult>> {
        self.last(&pat.inner)
    }
    fn last(&self, m: &PathMatcher) -> Result<MockRequest, Box<EvalAltResult>> {
        self.inner
            .last_request(m)
            .map(MockRequest::new)
            .ok_or_else(|| format!("no request recorded on `{}`", m.label()).into())
    }
    /// All requests whose path matches, in arrival order.
    pub(super) fn requests(&mut self, path: &str) -> Dynamic {
        self.all(&PathMatcher::Exact(path.to_string()))
    }
    pub(super) fn requests_re(&mut self, pat: PathPattern) -> Dynamic {
        self.all(&pat.inner)
    }
    fn all(&self, m: &PathMatcher) -> Dynamic {
        let arr: rhai::Array = self
            .inner
            .requests(m)
            .into_iter()
            .map(|r| Dynamic::from(MockRequest::new(r)))
            .collect();
        Dynamic::from_array(arr)
    }
    /// Stop the server early (it otherwise stops at scenario teardown).
    pub(super) fn stop(&mut self) {
        self.inner.shutdown();
    }
}

/// A path matcher passed to `respond`/`on`/`request_count`/…: either an exact path
/// (a plain string) or a regex built with `regex(...)`.
#[derive(Clone)]
pub(super) struct PathPattern {
    pub(super) inner: PathMatcher,
}

/// A request the mock server received, exposed to scenarios. Mirrors
/// [`HttpResponse`]'s accessors (`json`, `header`, plus `method`/`path`/`body`).
#[derive(Clone)]
pub(super) struct MockRequest {
    inner: Arc<EngineMockRequest>,
}

impl MockRequest {
    pub(super) fn new(req: EngineMockRequest) -> Self {
        Self {
            inner: Arc::new(req),
        }
    }
    pub(super) fn method(&mut self) -> String {
        mark_pending_label("request method");
        self.inner.method.clone()
    }
    pub(super) fn path(&mut self) -> String {
        mark_pending_label("request path");
        self.inner.path.clone()
    }
    pub(super) fn body(&mut self) -> String {
        mark_pending_label("request body");
        self.inner.body.clone()
    }
    /// A request header value (lower-case lookup), or `()` if absent.
    pub(super) fn header(&mut self, name: &str) -> Dynamic {
        mark_pending_label(format!("request header {name}"));
        convert::opt_to_dynamic(self.inner.headers.get(&name.to_lowercase()).cloned())
    }
    /// A query parameter value, or `()` if absent.
    pub(super) fn query(&mut self, name: &str) -> Dynamic {
        mark_pending_label(format!("request query {name}"));
        convert::opt_to_dynamic(self.inner.query.get(name).cloned())
    }
    /// The value at a dotted JSON path in the body (`""` for the whole body), typed
    /// like `HttpResponse::json` (object→map, array, number, bool, `null`→`()`).
    pub(super) fn json(&mut self, path: &str) -> Result<Dynamic, Box<EvalAltResult>> {
        mark_pending_label(if path.is_empty() {
            "request body (JSON)".to_string()
        } else {
            format!("request {path}")
        });
        json_path_value(&self.inner.body, path)
            .map(|v| convert::json_to_dynamic(&v))
            .map_err(|e| e.to_string().into())
    }
}
