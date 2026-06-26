//! Wire the Rhai engine: register the `State` namespace, the `Agent`/`Assertion`/
//! `HttpResponse`/`AudioSpec` value types with their verbs, and the global
//! functions. Every verb converts Rhai values and delegates to [`crate::engine`].

use super::convert;
use super::host::{Registry, SkipMarker};
use super::types::{Agent, Assertion, HttpMock, HttpResponse, MockRequest, PathPattern, Peer};
use crate::engine::audio::AudioSpec;
use crate::engine::ctx::{CallState, Ctx};
use crate::engine::mock_server::{self, PathMatcher, Responder};
use crate::engine::{ScenarioInfo, assertion, audio, http};
use crate::runtime::report::Event;
use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, Map, NativeCallContext, Position};
use std::sync::Arc;
use std::time::Duration;

/// The per-file env map (`--env-file` + sibling `<scenario>.env`), mutable so
/// `load_env(...)` can extend it at run time.
pub(super) type EnvVars = Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>;

pub(super) fn build_engine(
    ctx: Arc<Ctx>,
    registry: Arc<Registry>,
    env: EnvVars,
    base_dir: std::path::PathBuf,
) -> Engine {
    let mut engine = Engine::new();

    // Hardening (defense in depth — scenarios are trusted, but cap footguns).
    // Rhai has no fs/network access by default; we additionally forbid `eval` and
    // bound recursion, expression nesting and total operations so a runaway loop
    // fails instead of hanging.
    engine.disable_symbol("eval");
    engine.set_max_call_levels(64);
    engine.set_max_expr_depths(64, 64);
    engine.set_max_operations(50_000_000);

    register_state(&mut engine);
    register_agent(&mut engine, &ctx);
    register_assertions(&mut engine, &ctx);
    register_http(&mut engine, &ctx);
    register_mock(&mut engine, &ctx, &registry);
    register_audio(&mut engine, &ctx);
    register_globals(&mut engine, &ctx, &registry, &env, base_dir);

    engine
}

/// `CallState` type + the `State::Idle`/`Ringing`/`Established` namespace.
fn register_state(engine: &mut Engine) {
    engine.register_type_with_name::<CallState>("CallState");
    engine.register_fn("==", |a: CallState, b: CallState| a == b);
    engine.register_fn("!=", |a: CallState, b: CallState| a != b);

    // A static module is global — unlike scope constants it is visible inside
    // named `fn`s too — and namespacing keeps the names out of the bare scope.
    let mut state = rhai::Module::new();
    state.set_var("Idle", CallState::Idle);
    state.set_var("Ringing", CallState::Ringing);
    state.set_var("Established", CallState::Established);
    engine.register_static_module("State", state.into());
}

fn register_agent(engine: &mut Engine, ctx: &Arc<Ctx>) {
    engine.register_type_with_name::<Agent>("Agent");
    reg!(
        engine,
        "get$registered",
        ["agent: Agent", "bool"],
        "/// Whether the agent's account is currently registered.",
        Agent::registered
    );
    reg!(
        engine,
        "get$state",
        ["agent: Agent", "CallState"],
        "/// The agent's current call phase: `Idle`, `Ringing` or `Established`.",
        Agent::call_state
    );
    reg!(
        engine,
        "get$reason",
        ["agent: Agent", "?"],
        "/// The last closed call's reason (string), or `()` if none yet.\n\
         /// # Returns: string?",
        Agent::reason
    );
    reg!(
        engine,
        "get$status_code",
        ["agent: Agent", "?"],
        "/// SIP status code from the last closed call's reason (int, e.g. `603`),\n\
         /// or `()` if the reason isn't a SIP response (local hangup, reset, …).\n\
         /// # Returns: int?",
        Agent::status_code
    );
    reg!(
        engine,
        "header",
        ["agent: Agent", "name: string", "?"],
        "/// Value of a header on a received INVITE (string), or `()` if absent.\n\
         /// # Returns: string?",
        Agent::header
    );
    reg!(
        engine,
        "get$peer",
        ["agent: Agent", "Peer"],
        "/// The current call's remote party (the caller for an incoming call); read\n\
         /// `peer.uri` / `peer.number` / `peer.name` (each `()` if there's no call).",
        Agent::peer
    );
    engine.register_type_with_name::<Peer>("Peer");
    reg!(
        engine,
        "get$uri",
        ["peer: Peer", "?"],
        "/// The remote party's full URI (e.g. `sip:bob@example.com`), or `()`.\n\
         /// # Returns: string?",
        Peer::uri
    );
    reg!(
        engine,
        "get$number",
        ["peer: Peer", "?"],
        "/// The remote party's number (user-part of the URI), or `()`.\n\
         /// # Returns: string?",
        Peer::number
    );
    reg!(
        engine,
        "get$name",
        ["peer: Peer", "?"],
        "/// The remote party's display name, or `()` if absent.\n\
         /// # Returns: string?",
        Peer::name
    );
    engine.register_fn("to_string", Peer::display);
    reg!(
        engine,
        "headers",
        ["agent: Agent", "map"],
        "/// All received INVITE headers as a map (name → value); duplicates collapse,\n\
         /// use `header(name)` for a specific one.",
        Agent::headers
    );
    reg!(
        engine,
        "info",
        ["agent: Agent", "map"],
        "/// A map of the agent's current state: name, aor, registered, state,\n\
         /// reason, status_code, calls. Handy to `print(...)` or assert on.",
        Agent::info
    );
    reg!(
        engine,
        "to_json",
        ["agent: Agent", "string"],
        "/// The agent's current state as a JSON string (for `log(...)`/debugging).",
        Agent::to_json
    );
    reg!(
        engine,
        "register",
        ["agent: Agent", "()"],
        "/// (Re-)register the agent's account.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// a.register();\n\
         /// await_until(|| assert(a.registered).is_true(), \"10s\");\n\
         /// ```",
        Agent::register
    );
    reg!(
        engine,
        "accept",
        ["agent: Agent", "()"],
        "/// Answer the agent's incoming call.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// await_until(|| assert(b.state).equals(State::Ringing), \"15s\");\n\
         /// b.accept();\n\
         /// ```",
        Agent::accept
    );
    reg!(
        engine,
        "hangup",
        ["agent: Agent", "()"],
        "/// Hang up the agent's active call.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// a.hangup();\n\
         /// await_until(|| assert(a.state).equals(State::Idle), \"10s\");\n\
         /// ```",
        Agent::hangup
    );
    reg!(
        engine,
        "hold",
        ["agent: Agent", "()"],
        "/// Put the active call on hold.",
        Agent::hold
    );
    reg!(
        engine,
        "resume",
        ["agent: Agent", "()"],
        "/// Resume a held call.",
        Agent::resume
    );
    reg!(
        engine,
        "mute",
        ["agent: Agent", "()"],
        "/// Toggle mute on the active call.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// a.mute(); // mute; call again to unmute\n\
         /// ```",
        Agent::mute
    );
    reg!(
        engine,
        "dtmf",
        ["agent: Agent", "digits: string", "()"],
        "/// Send DTMF tones (characters `0-9`, `*`, `#`, `A-D`) back-to-back.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// a.dtmf(\"123#\");\n\
         /// ```",
        Agent::dtmf
    );
    reg!(
        engine,
        "dtmf",
        ["agent: Agent", "digits: string", "gap: string", "()"],
        "/// Send DTMF tones with a pause between digits.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// a.dtmf(\"123#\", \"200ms\");\n\
         /// ```",
        Agent::dtmf_spaced
    );
    reg!(
        engine,
        "dial",
        ["agent: Agent", "target: Agent", "()"],
        "/// Dial another agent at its AOR.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// a.dial(b);                 // dial agent B at its AOR\n\
         /// a.dial(\"+49301234567\");    // …or a number/URI in A's domain\n\
         /// await_until(|| assert(b.state).equals(State::Ringing), \"15s\");\n\
         /// ```",
        Agent::dial_agent
    );
    reg!(
        engine,
        "dial",
        ["agent: Agent", "target: string", "()"],
        "/// Dial a literal SIP URI, or a bare number/extension in the agent's own domain.",
        Agent::dial_uri
    );
    reg!(
        engine,
        "transfer",
        ["agent: Agent", "target: Agent", "()"],
        "/// Blind-transfer (REFER) the active call to another agent's AOR.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// callee.transfer(target); // hand the caller off to `target`\n\
         /// ```",
        Agent::transfer_agent
    );
    reg!(
        engine,
        "transfer",
        ["agent: Agent", "target: string", "()"],
        "/// Blind-transfer (REFER) the active call to a literal URI or bare number.",
        Agent::transfer_uri
    );
    reg!(
        engine,
        "attended_transfer",
        ["agent: Agent", "target: Agent", "()"],
        "/// Start an attended transfer: place a consultation call to another agent.\n\
         /// Complete it with `complete_transfer()` once that call is established.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// callee.attended_transfer(target);   // consult `target`\n\
         /// await_until(|| assert(target.state).equals(State::Established));\n\
         /// callee.complete_transfer();         // connect caller and target\n\
         /// ```",
        Agent::attended_transfer_agent
    );
    reg!(
        engine,
        "attended_transfer",
        ["agent: Agent", "target: string", "()"],
        "/// Start an attended transfer to a literal URI or bare number.",
        Agent::attended_transfer_uri
    );
    reg!(
        engine,
        "complete_transfer",
        ["agent: Agent", "()"],
        "/// Complete the pending attended transfer (REFER with Replaces).",
        Agent::complete_transfer
    );
    reg!(
        engine,
        "abort_transfer",
        ["agent: Agent", "()"],
        "/// Abort the pending attended transfer.",
        Agent::abort_transfer
    );

    // agent("Name", #{ … }) → connect a real session, store it, return a handle.
    let c = ctx.clone();
    reg!(
        engine,
        "agent",
        ["name: string", "config: map", "Agent"],
        // Doc comment (with the config options table) is generated from the schema in
        // `convert::AGENT_CONFIG`, the same source `account_from_map` validates against.
        convert::agent_config_doc(),
        move |name: &str, config: Map| -> Result<Agent, Box<EvalAltResult>> {
            let account = convert::account_from_map(name, &config)?;
            let headers = convert::headers_from_map(&config)?;
            c.connect_agent(name, account, &headers)
                .map_err(|e| -> Box<EvalAltResult> { e.into() })?;
            Ok(Agent {
                name: name.to_string(),
                ctx: c.clone(),
            })
        }
    );
}

/// Fluent assertions (`assert(x).equals(y)`) + `await_until`.
fn register_assertions(engine: &mut Engine, ctx: &Arc<Ctx>) {
    reg!(
        engine,
        "to_string",
        ["state: CallState", "string"],
        "/// The call state as a string.",
        |c: CallState| c.to_string()
    );

    let c = ctx.clone();
    reg!(
        engine,
        "assert",
        ["actual: ?", "Assertion"],
        "/// Begin a fluent assertion on a value: `assert(x).equals(y)`, `.is_true()`,\n\
         /// `.greater_than(n)`, etc. Matchers chain (`.at_least(200).at_most(299)`)\n\
         /// and error (with a value-based message) on a mismatch. Asserting on a\n\
         /// getter auto-labels the log line (`assert(caller.state)` → `Caller state`,\n\
         /// `assert(res.status)` → `HTTP status`); `.describe(…)` overrides.",
        move |actual: Dynamic| Assertion::new(c.clone(), actual)
    );
    reg!(
        engine,
        "describe",
        ["a: Assertion", "label: string", "Assertion"],
        "/// Label this assertion so the log line names it: `assert(caller.registered)\n\
         /// .describe(\"caller registered\").is_true()` → `caller registered: ✓ expect …`.",
        Assertion::describe
    );
    reg!(
        engine,
        "value",
        ["a: Assertion", "?"],
        "/// The value under assertion, so a verified value can be bound.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// let id = await_until(|| assert(callee.header(\"X-Id\")).is_present().value());\n\
         /// ```",
        Assertion::value
    );

    // Matchers return the Assertion (chainable).
    reg!(
        engine,
        "equals",
        ["a: Assertion", "expected: ?", "Assertion"],
        "/// Assert the value equals `expected` (`is` is a reserved word in Rhai).\n\
         /// # Example\n\
         /// ```rhai\n\
         /// assert(a.state).equals(State::Established);\n\
         /// ```",
        Assertion::equals
    );
    reg!(
        engine,
        "not_equals",
        ["a: Assertion", "expected: ?", "Assertion"],
        "/// Assert the value does not equal `expected`.",
        Assertion::not_equals
    );
    reg!(
        engine,
        "is_true",
        ["a: Assertion", "Assertion"],
        "/// Assert the value is `true`.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// assert(a.registered).is_true();\n\
         /// ```",
        Assertion::is_true
    );
    reg!(
        engine,
        "is_false",
        ["a: Assertion", "Assertion"],
        "/// Assert the value is `false`.",
        Assertion::is_false
    );
    reg!(
        engine,
        "is_present",
        ["a: Assertion", "Assertion"],
        "/// Assert the value is present (not `()`), e.g. a received header.",
        Assertion::is_present
    );
    reg!(
        engine,
        "is_absent",
        ["a: Assertion", "Assertion"],
        "/// Assert the value is absent (`()`).",
        Assertion::is_absent
    );
    reg!(
        engine,
        "is_empty",
        ["a: Assertion", "Assertion"],
        "/// Assert the string/array/map value is empty.",
        Assertion::is_empty
    );
    reg!(
        engine,
        "is_not_empty",
        ["a: Assertion", "Assertion"],
        "/// Assert the string/array/map value is not empty.",
        Assertion::is_not_empty
    );
    reg!(
        engine,
        "contains",
        ["a: Assertion", "needle: string", "Assertion"],
        "/// Assert the (string) value contains `needle`.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// assert(a.header(\"User-Agent\")).contains(\"baresip\");\n\
         /// ```",
        Assertion::contains
    );
    reg!(
        engine,
        "matches",
        ["a: Assertion", "pattern: string", "Assertion"],
        "/// Assert the (string) value matches the regex `pattern`.",
        Assertion::matches
    );
    reg!(
        engine,
        "greater_than",
        ["a: Assertion", "n: int", "Assertion"],
        "/// Assert the (numeric) value is > `n`.",
        Assertion::greater_than
    );
    reg!(
        engine,
        "at_least",
        ["a: Assertion", "n: int", "Assertion"],
        "/// Assert the (numeric) value is >= `n`.",
        Assertion::at_least
    );
    reg!(
        engine,
        "less_than",
        ["a: Assertion", "n: int", "Assertion"],
        "/// Assert the (numeric) value is < `n`.",
        Assertion::less_than
    );
    reg!(
        engine,
        "at_most",
        ["a: Assertion", "n: int", "Assertion"],
        "/// Assert the (numeric) value is <= `n`.",
        Assertion::at_most
    );

    let c = ctx.clone();
    reg!(
        engine,
        "await_until",
        ["body: Fn", "?"],
        "/// Re-run the expression until its assertion holds or the default timeout\n\
         /// elapses; returns the body's value, so `.value()` can bind a verified value.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// await_until(|| assert(a.registered).is_true());\n\
         /// ```",
        move |nctx: NativeCallContext, body: FnPtr| -> Result<Dynamic, Box<EvalAltResult>> {
            await_until(&c, &nctx, &body, c.default_timeout())
        }
    );
    let c = ctx.clone();
    reg!(
        engine,
        "await_until",
        ["body: Fn", "within: string", "?"],
        "/// Like `await_until(body)` but with an explicit timeout, e.g. `\"15s\"`.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// await_until(|| assert(b.state).equals(State::Ringing), \"15s\");\n\
         /// ```",
        move |nctx: NativeCallContext,
              body: FnPtr,
              within: &str|
              -> Result<Dynamic, Box<EvalAltResult>> {
            let timeout = crate::engine::duration::parse_duration(within)?;
            await_until(&c, &nctx, &body, timeout)
        }
    );
}

/// Drive the neutral poll loop with a Rhai closure, returning the body's value
/// (so `await_until(|| assert(x).is_present().value())` binds it).
fn await_until(
    ctx: &Arc<Ctx>,
    nctx: &NativeCallContext,
    body: &FnPtr,
    timeout: Duration,
) -> Result<Dynamic, Box<EvalAltResult>> {
    let slot = std::cell::RefCell::new(Dynamic::UNIT);
    let outcome = assertion::await_until(
        ctx,
        || match body.call_within_context::<Dynamic>(nctx, ()) {
            Ok(v) => {
                *slot.borrow_mut() = v;
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        },
        timeout,
    );
    outcome.map(|()| slot.into_inner()).map_err(|e| e.into())
}

fn register_http(engine: &mut Engine, ctx: &Arc<Ctx>) {
    engine.register_type_with_name::<HttpResponse>("HttpResponse");
    reg!(
        engine,
        "get$status",
        ["response: HttpResponse", "int"],
        "/// The HTTP response status code.",
        HttpResponse::status
    );
    reg!(
        engine,
        "get$body",
        ["response: HttpResponse", "string"],
        "/// The HTTP response body as a string.",
        HttpResponse::body
    );
    reg!(
        engine,
        "header",
        ["response: HttpResponse", "name: string", "?"],
        "/// A response header value (string), or `()` if absent.\n\
         /// # Returns: string?",
        HttpResponse::header
    );
    reg!(
        engine,
        "json",
        ["response: HttpResponse", "path: string", "?"],
        "/// The value at a dotted JSON path (e.g. `\"data.id\"`), typed: object→map,\n\
         /// array, number, bool, `null`→`()`. Errors if the path is missing.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// assert(res.json(\"data.id\")).equals(42);\n\
         /// ```",
        HttpResponse::json
    );
    reg!(
        engine,
        "json",
        ["response: HttpResponse", "?"],
        "/// The whole JSON body as a native value (object→map, array, …).",
        HttpResponse::json_all
    );
    reg!(
        engine,
        "expect_status",
        ["response: HttpResponse", "code: int", "()"],
        "/// Assert and report the status; errors on mismatch.",
        HttpResponse::expect_status
    );

    let c = ctx.clone();
    reg!(
        engine,
        "http",
        ["method: string", "url: string", "HttpResponse"],
        "/// Make an HTTP request and return the response.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// let res = http(\"GET\", env(\"API_URL\") + \"/calls\");\n\
         /// res.expect_status(200);\n\
         /// ```",
        move |method: &str, url: &str| -> Result<HttpResponse, Box<EvalAltResult>> {
            let inner = http::perform(&c, method, url, &[], None)
                .map_err(|e| -> Box<EvalAltResult> { e.into() })?;
            Ok(HttpResponse { inner })
        }
    );
    let c = ctx.clone();
    reg!(
        engine,
        "http",
        [
            "method: string",
            "url: string",
            "options: map",
            "HttpResponse"
        ],
        // Doc + options table generated from `convert::HTTP_OPTIONS` (same source the
        // request validates against).
        convert::http_options_doc(),
        move |method: &str, url: &str, opts: Map| -> Result<HttpResponse, Box<EvalAltResult>> {
            convert::reject_unknown_keys("http options", &opts, convert::HTTP_OPTIONS)?;
            let headers = opts
                .get("headers")
                .and_then(|d| d.clone().try_cast::<Map>())
                .map(|h| {
                    h.iter()
                        .filter_map(|(k, v)| {
                            v.clone().into_string().ok().map(|val| (k.to_string(), val))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let body = opts.get("body").and_then(convert::body_to_string);
            let inner = http::perform(&c, method, url, &headers, body)
                .map_err(|e| -> Box<EvalAltResult> { e.into() })?;
            Ok(HttpResponse { inner })
        }
    );
}

/// The mock HTTP server: `mock_server(...)`, its `respond`/`on` route verbs, the
/// `MockRequest` accessors, and the `json_response`/`text_response` helpers. Webhook
/// e2e tests start a server, register what to answer, drive the call, then assert on
/// the requests it received (all waiting goes through `await_until`).
/// `"*"` (any case) means "match any method"; otherwise the upper-cased method.
fn method_opt(method: &str) -> Option<String> {
    (method != "*").then(|| method.to_uppercase())
}

/// Register a static response on `mock` for `method`/`path`.
fn add_static(
    mock: &HttpMock,
    method: Option<String>,
    path: PathMatcher,
    response: Map,
) -> Result<(), Box<EvalAltResult>> {
    let resp = convert::map_to_response(&response)?;
    mock.inner.set_route(method, path, Responder::Static(resp));
    Ok(())
}

/// Register a dynamic (Rhai-closure) responder on `mock` for `method`/`path`.
fn add_dynamic(
    registry: &Registry,
    mock: &HttpMock,
    method: Option<String>,
    path: PathMatcher,
    responder: FnPtr,
) -> Result<(), Box<EvalAltResult>> {
    let r = build_dynamic_responder(registry, responder)?;
    mock.inner.set_route(method, path, r);
    Ok(())
}

/// Turn a Rhai responder closure into an engine [`Responder`]: detach its captured
/// variables (like `parallel`, so it can run on a worker thread without aliasing the
/// script thread's values), then call it per request. The engine is held weakly so
/// the responder (reachable from `Ctx`) can't form a reference cycle with it. A bad
/// return or a Rhai error becomes an `Err`, which the server logs and answers `500`.
fn build_dynamic_responder(
    registry: &Registry,
    mut responder: FnPtr,
) -> Result<Responder, Box<EvalAltResult>> {
    let (engine, ast) = registry
        .exec()
        .ok_or_else(|| -> Box<EvalAltResult> { "on: engine not ready".into() })?;
    for v in responder.iter_curry_mut() {
        *v = v.flatten_clone();
    }
    let weak_engine = Arc::downgrade(&engine);
    let func = move |req: mock_server::MockRequest| -> Result<mock_server::MockResponse, String> {
        let Some(engine) = weak_engine.upgrade() else {
            return Err("scenario engine gone".into());
        };
        let arg = Dynamic::from(MockRequest::new(req));
        match responder.call::<Dynamic>(&engine, &ast, (arg,)) {
            Ok(d) => match d.try_cast::<Map>() {
                Some(m) => convert::map_to_response(&m).map_err(|e| e.to_string()),
                None => Err(
                    "responder must return a response map (use json_response/text_response)".into(),
                ),
            },
            Err(e) => Err(format!("responder failed: {e}")),
        }
    };
    Ok(Responder::Dynamic(Box::new(func)))
}

fn register_mock(engine: &mut Engine, ctx: &Arc<Ctx>, registry: &Arc<Registry>) {
    engine.register_type_with_name::<HttpMock>("HttpMock");
    engine.register_type_with_name::<MockRequest>("MockRequest");
    engine.register_type_with_name::<PathPattern>("PathPattern");

    reg!(
        engine,
        "regex",
        ["pattern: string", "PathPattern"],
        "/// A regex path matcher for `respond`/`on`/`request_count`/… anchored to the\n\
         /// whole path (`/calls/.*` matches `/calls/123`). Errors on a bad pattern.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// hooks.on(regex(\"/calls/.*\"), |req| text_response(\"ok\"));\n\
         /// ```",
        |pattern: &str| -> Result<PathPattern, Box<EvalAltResult>> {
            PathMatcher::regex(pattern)
                .map(|inner| PathPattern { inner })
                .map_err(|e| e.into())
        }
    );

    // ── starting a server ──
    let c = ctx.clone();
    reg!(
        engine,
        "mock_server",
        ["HttpMock"],
        "/// Start a mock HTTP server on a free port and return a handle. Stopped\n\
         /// automatically at the end of the scenario. Use `url` to point the system\n\
         /// under test at it, `respond`/`on` to define routes.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// let hooks = mock_server();\n\
         /// hooks.on(\"POST\", \"/voice\", |req| json_response(#{ actions: [ #{ type: \"answer\" } ] }));\n\
         /// http(\"PUT\", env(\"API_URL\") + \"/config?webhook=\" + hooks.url + \"/voice\");\n\
         /// ```",
        move || -> Result<HttpMock, Box<EvalAltResult>> {
            let inner =
                mock_server::start(&c, None).map_err(|e| -> Box<EvalAltResult> { e.into() })?;
            c.register_mock(inner.clone());
            Ok(HttpMock { inner })
        }
    );
    let c = ctx.clone();
    reg!(
        engine,
        "mock_server",
        ["config: map", "HttpMock"],
        // Doc + config table generated from `convert::MOCK_SERVER_CONFIG`.
        convert::mock_server_config_doc(),
        move |config: Map| -> Result<HttpMock, Box<EvalAltResult>> {
            convert::reject_unknown_keys("mock_server", &config, convert::MOCK_SERVER_CONFIG)?;
            let port = match config.get("port") {
                Some(d) => Some(
                    u16::try_from(d.as_int().map_err(|_| -> Box<EvalAltResult> {
                        "mock_server: `port` must be an integer".into()
                    })?)
                    .map_err(|_| -> Box<EvalAltResult> {
                        "mock_server: `port` out of range".into()
                    })?,
                ),
                None => None,
            };
            let inner =
                mock_server::start(&c, port).map_err(|e| -> Box<EvalAltResult> { e.into() })?;
            c.register_mock(inner.clone());
            Ok(HttpMock { inner })
        }
    );

    // ── server handle: getters + verbs ──
    reg!(
        engine,
        "get$url",
        ["mock: HttpMock", "string"],
        "/// The server's base URL, e.g. `http://127.0.0.1:8080`.",
        HttpMock::url
    );
    reg!(
        engine,
        "get$port",
        ["mock: HttpMock", "int"],
        "/// The port the server is listening on.",
        HttpMock::port
    );
    reg!(
        engine,
        "respond",
        [
            "mock: HttpMock",
            "method: string",
            "path: string",
            "response: map",
            "()"
        ],
        "/// Register a static response for `method path`: a map\n\
         /// `#{ status: 200, content_type: \"…\", headers: #{…}, body: <string|map> }`\n\
         /// (use `json_response`/`text_response` to build it). `method` may be `\"*\"`\n\
         /// for any method. Re-register to stage the next answer between webhooks.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// hooks.respond(\"POST\", \"/voice\", json_response(#{ actions: [ #{ type: \"hangup\" } ] }));\n\
         /// ```",
        |mock: &mut HttpMock, method: &str, path: &str, response: Map| {
            add_static(
                mock,
                method_opt(method),
                PathMatcher::Exact(path.into()),
                response,
            )
        }
    );
    reg!(
        engine,
        "respond",
        [
            "mock: HttpMock",
            "method: string",
            "path: PathPattern",
            "response: map",
            "()"
        ],
        "/// Static response for `method` and a `regex(...)` path.",
        |mock: &mut HttpMock, method: &str, path: PathPattern, response: Map| {
            add_static(mock, method_opt(method), path.inner, response)
        }
    );
    reg!(
        engine,
        "respond",
        ["mock: HttpMock", "path: string", "response: map", "()"],
        "/// Static response for `path` on any HTTP method.",
        |mock: &mut HttpMock, path: &str, response: Map| {
            add_static(mock, None, PathMatcher::Exact(path.into()), response)
        }
    );
    reg!(
        engine,
        "respond",
        ["mock: HttpMock", "path: PathPattern", "response: map", "()"],
        "/// Static response for a `regex(...)` path on any HTTP method.",
        |mock: &mut HttpMock, path: PathPattern, response: Map| {
            add_static(mock, None, path.inner, response)
        }
    );
    reg!(
        engine,
        "request_count",
        ["mock: HttpMock", "path: string", "int"],
        "/// How many requests arrived on `path` (any method). Poll it with\n\
         /// `await_until` to wait for a webhook.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// await_until(|| assert(hooks.request_count(\"/voice\")).equals(1), \"10s\");\n\
         /// ```",
        HttpMock::request_count
    );
    reg!(
        engine,
        "request_count",
        ["mock: HttpMock", "path: PathPattern", "int"],
        "/// How many requests arrived on a `regex(...)` path (any method).",
        HttpMock::request_count_re
    );
    reg!(
        engine,
        "last_request",
        ["mock: HttpMock", "path: string", "MockRequest"],
        "/// The most recent request on `path` (errors if none yet). Read it after\n\
         /// `await_until` confirms the webhook arrived.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// let req = hooks.last_request(\"/voice\");\n\
         /// assert(req.json(\"event\")).equals(\"incoming_call\");\n\
         /// ```",
        HttpMock::last_request
    );
    reg!(
        engine,
        "last_request",
        ["mock: HttpMock", "path: PathPattern", "MockRequest"],
        "/// The most recent request on a `regex(...)` path (errors if none yet).",
        HttpMock::last_request_re
    );
    reg!(
        engine,
        "requests",
        ["mock: HttpMock", "path: string", "array"],
        "/// All requests received on `path`, in arrival order, as `MockRequest`s.",
        HttpMock::requests
    );
    reg!(
        engine,
        "requests",
        ["mock: HttpMock", "path: PathPattern", "array"],
        "/// All requests on a `regex(...)` path, in arrival order, as `MockRequest`s.",
        HttpMock::requests_re
    );
    reg!(
        engine,
        "stop",
        ["mock: HttpMock", "()"],
        "/// Stop the server now (it otherwise stops automatically at scenario end).",
        HttpMock::stop
    );

    // on([method,] path, |req| <response map>) — a dynamic responder evaluated per
    // request on a runtime worker. The closure must be pure (request → response, no
    // agent verbs): it runs concurrently with the script thread. `method` may be
    // `"*"` (or be omitted) to match any method; `path` may be a `regex(...)`.
    let r = registry.clone();
    reg!(
        engine,
        "on",
        [
            "mock: HttpMock",
            "method: string",
            "path: string",
            "responder: Fn",
            "()"
        ],
        "/// Answer `method path` dynamically: the `|req|` closure receives the\n\
         /// `MockRequest` and returns a response map (e.g. `json_response(#{…})`).\n\
         /// `method` may be `\"*\"` for any method. The closure runs on a runtime\n\
         /// worker, so keep it pure (request → response): no agent verbs, no `wait`\n\
         /// — those block a worker thread.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// hooks.on(\"POST\", \"/voice\", |req| {\n\
         ///     if req.json(\"event\") == \"incoming_call\" {\n\
         ///         json_response(#{ actions: [ #{ type: \"answer\" } ] })\n\
         ///     } else {\n\
         ///         json_response(#{ actions: [ #{ type: \"hangup\" } ] })\n\
         ///     }\n\
         /// });\n\
         /// ```",
        move |mock: &mut HttpMock, method: &str, path: &str, responder: FnPtr| {
            add_dynamic(
                &r,
                mock,
                method_opt(method),
                PathMatcher::Exact(path.into()),
                responder,
            )
        }
    );
    let r = registry.clone();
    reg!(
        engine,
        "on",
        [
            "mock: HttpMock",
            "method: string",
            "path: PathPattern",
            "responder: Fn",
            "()"
        ],
        "/// Dynamic responder for `method` and a `regex(...)` path.",
        move |mock: &mut HttpMock, method: &str, path: PathPattern, responder: FnPtr| {
            add_dynamic(&r, mock, method_opt(method), path.inner, responder)
        }
    );
    let r = registry.clone();
    reg!(
        engine,
        "on",
        ["mock: HttpMock", "path: string", "responder: Fn", "()"],
        "/// Dynamic responder for `path` on any HTTP method.",
        move |mock: &mut HttpMock, path: &str, responder: FnPtr| {
            add_dynamic(&r, mock, None, PathMatcher::Exact(path.into()), responder)
        }
    );
    let r = registry.clone();
    reg!(
        engine,
        "on",
        ["mock: HttpMock", "path: PathPattern", "responder: Fn", "()"],
        "/// Dynamic responder for a `regex(...)` path on any HTTP method.",
        move |mock: &mut HttpMock, path: PathPattern, responder: FnPtr| {
            add_dynamic(&r, mock, None, path.inner, responder)
        }
    );

    // ── MockRequest accessors (mirror HttpResponse) ──
    reg!(
        engine,
        "get$method",
        ["request: MockRequest", "string"],
        "/// The request method (upper-case).",
        MockRequest::method
    );
    reg!(
        engine,
        "get$path",
        ["request: MockRequest", "string"],
        "/// The request path.",
        MockRequest::path
    );
    reg!(
        engine,
        "get$body",
        ["request: MockRequest", "string"],
        "/// The raw request body.",
        MockRequest::body
    );
    reg!(
        engine,
        "header",
        ["request: MockRequest", "name: string", "?"],
        "/// A request header value (case-insensitive), or `()` if absent.\n\
         /// # Returns: string?",
        MockRequest::header
    );
    reg!(
        engine,
        "query",
        ["request: MockRequest", "name: string", "?"],
        "/// A query-string parameter value, or `()` if absent.\n\
         /// # Returns: string?",
        MockRequest::query
    );
    reg!(
        engine,
        "json",
        ["request: MockRequest", "path: string", "?"],
        "/// The value at a dotted JSON path in the body (object→map, array, number,\n\
         /// bool, `null`→`()`). Errors if the path is missing.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// assert(req.json(\"call.from\")).equals(\"+49301234567\");\n\
         /// ```",
        MockRequest::json
    );

    // ── response-map helpers ──
    reg!(
        engine,
        "json_response",
        ["body: ?", "map"],
        "/// Build a `200 application/json` response map from `body` (JSON-encoded),\n\
         /// for `respond`/`on`. `body` may be a map or an array.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// hooks.respond(\"POST\", \"/voice\", json_response(#{ actions: [ #{ type: \"answer\" } ] }));\n\
         /// ```",
        |body: Dynamic| -> Map {
            let mut m = Map::new();
            m.insert("status".into(), Dynamic::from(200_i64));
            m.insert("content_type".into(), "application/json".into());
            let json = serde_json::to_string(&convert::dynamic_to_json(&body))
                .unwrap_or_else(|_| "null".into());
            m.insert("body".into(), json.into());
            m
        }
    );
    reg!(
        engine,
        "text_response",
        ["body: string", "map"],
        "/// Build a `200 text/plain` response map from `body`, for `respond`/`on`.",
        |body: &str| -> Map {
            let mut m = Map::new();
            m.insert("status".into(), Dynamic::from(200_i64));
            m.insert("content_type".into(), "text/plain".into());
            m.insert("body".into(), body.to_string().into());
            m
        }
    );
}

fn register_audio(engine: &mut Engine, ctx: &Arc<Ctx>) {
    engine.register_type_with_name::<AudioSpec>("AudioSpec");
    reg!(
        engine,
        "tone",
        ["freq: int", "AudioSpec"],
        "/// A sine-tone audio source at the given frequency (Hz), for `send_audio`.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// a.send_audio(tone(440));\n\
         /// ```",
        |freq: i64| AudioSpec::Tone(freq.max(0) as u32)
    );
    reg!(
        engine,
        "file",
        ["path: string", "AudioSpec"],
        "/// A WAV-file audio source, for `send_audio`.",
        |path: &str| AudioSpec::File(path.to_string())
    );
    reg!(
        engine,
        "silent",
        ["AudioSpec"],
        "/// A silent audio source (stop sending), for `send_audio`.",
        || AudioSpec::Silent
    );

    let c = ctx.clone();
    reg!(
        engine,
        "send_audio",
        ["agent: Agent", "source: AudioSpec", "()"],
        "/// Switch the agent's active-call audio source: `tone(Hz)`, `file(path)` or `silent()`.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// a.send_audio(tone(440));         // play a 440 Hz tone\n\
         /// a.send_audio(file(\"prompt.wav\"));\n\
         /// ```",
        move |agent: Agent, spec: AudioSpec| -> Result<(), Box<EvalAltResult>> {
            audio::send_audio(&c, &agent.name, spec).map_err(|e| e.into())
        }
    );
    let c = ctx.clone();
    reg!(
        engine,
        "verify_audio",
        ["agent: Agent", "freq: int", "within: string", "()"],
        "/// Assert the agent is receiving a tone at `freq` Hz within the window (Goertzel).\n\
         /// # Example\n\
         /// ```rhai\n\
         /// a.send_audio(tone(440));\n\
         /// b.verify_audio(440, \"5s\"); // b hears A's 440 Hz tone\n\
         /// ```",
        move |agent: Agent, freq: i64, within: &str| -> Result<(), Box<EvalAltResult>> {
            audio::verify_audio(&c, &agent.name, freq, within).map_err(|e| e.into())
        }
    );
    let c = ctx.clone();
    reg!(
        engine,
        "verify_audio_connection",
        ["a: Agent", "b: Agent", "()"],
        "/// Assert two-way audio between two agents (a→b then b→a) at 1000 Hz.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// caller.verify_audio_connection(callee);\n\
         /// ```",
        move |a: Agent, b: Agent| -> Result<(), Box<EvalAltResult>> {
            audio::verify_audio_connection(&c, &a.name, &b.name).map_err(|e| e.into())
        }
    );
}

/// `default_timeout` / `wait` / `env` / `uuid` / `log` and the suite structure
/// (`scenario`/`setup`/`teardown`, which register into `registry`).
fn register_globals(
    engine: &mut Engine,
    ctx: &Arc<Ctx>,
    registry: &Arc<Registry>,
    env: &EnvVars,
    base_dir: std::path::PathBuf,
) {
    let c = ctx.clone();
    reg!(
        engine,
        "default_timeout",
        ["duration: string", "()"],
        "/// Set the default `await_until` timeout for the rest of the script (e.g. `\"10s\"`).",
        move |within: &str| -> Result<(), Box<EvalAltResult>> {
            c.set_default_timeout(crate::engine::duration::parse_duration(within)?);
            Ok(())
        }
    );

    let c = ctx.clone();
    reg!(
        engine,
        "wait",
        ["seconds: int", "()"],
        "/// Hold for N seconds; FAILS if a call that is established at the start drops.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// wait(3); // the call must stay up for 3s\n\
         /// ```",
        move |secs: i64| -> Result<(), Box<EvalAltResult>> {
            let secs = secs.max(0) as u64;
            c.emit(&Event::Wait {
                seconds: secs as f64,
            });
            // Snapshot the watch receivers, then release the sessions lock so it
            // isn't held across the (up to N-second) block_on.
            let watchers = {
                let sessions = c.sessions.lock().unwrap_or_else(|e| e.into_inner());
                sessions
                    .iter()
                    .map(|(name, s)| (name.clone(), s.state()))
                    .collect()
            };
            c.rt.block_on(crate::runtime::wait_holding(
                Duration::from_secs(secs),
                watchers,
            ))
            .map_err(|e| e.to_string().into())
        }
    );

    let e = env.clone();
    reg!(
        engine,
        "env",
        ["name: string", "string"],
        "/// Read a variable: first from `--env-file`/`<scenario>.env`/`load_env`, then\n\
         /// the process environment. Errors if unset. Use it for per-env credentials.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// let dom = env(\"SIP_DOMAIN\");\n\
         /// let a = agent(\"A\", #{ username: env(\"A_USER\"), domain: dom, password: env(\"A_PASS\") });\n\
         /// ```",
        move |name: &str| -> Result<String, Box<EvalAltResult>> {
            e.lock()
                .unwrap()
                .get(name)
                .cloned()
                .or_else(|| std::env::var(name).ok())
                .ok_or_else(|| format!("environment variable `{name}` is not set").into())
        }
    );

    // load_env("path.env") — merge a dotenv file into this file's env at run time,
    // resolved relative to the scenario's directory. Later loads win.
    let e = env.clone();
    reg!(
        engine,
        "load_env",
        ["path: string", "()"],
        "/// Load a dotenv file (`KEY=VALUE` lines) into `env(...)` for this scenario,\n\
         /// resolved relative to the scenario file. Later loads override earlier keys.",
        move |path: &str| -> Result<(), Box<EvalAltResult>> {
            let p = base_dir.join(path);
            super::merge_dotenv(&p, &mut e.lock().unwrap()).map_err(|err| err.to_string())?;
            Ok(())
        }
    );
    reg!(
        engine,
        "uuid",
        ["string"],
        "/// A fresh random UUID string.",
        || uuid::Uuid::new_v4().to_string()
    );

    let c = ctx.clone();
    reg!(
        engine,
        "log",
        ["message: string", "()"],
        "/// Print a timestamped note to the scenario log (and the `--json` stream),\n\
         /// unlike `print` which writes a bare line.",
        move |message: &str| c.emit(&Event::Log { message })
    );

    // parallel([|| …, || …]) — run zero-arg closures concurrently on worker
    // threads (Rhai eval is reentrant under the `sync` feature), wait for all,
    // and return their results. Fails if any task fails. For independent blocking
    // ops like `verify_audio` on several agents at once.
    let r = registry.clone();
    reg!(
        engine,
        "parallel",
        ["tasks: array", "array"],
        "/// Run the given zero-arg closures concurrently and wait for all; returns\n\
         /// their results as an array, and fails if any task fails. Use it for\n\
         /// independent blocking work, e.g. `verify_audio` on several agents at once.\n\
         /// Tasks may share captured variables (each gets an independent snapshot,\n\
         /// so they can't race). Don't overlap `await_until` across tasks; its\n\
         /// silencing is global.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// let results = parallel([\n\
         ///     || http(\"GET\", env(\"A_URL\")),\n\
         ///     || http(\"GET\", env(\"B_URL\")),\n\
         /// ]);\n\
         /// ```",
        move |tasks: rhai::Array| -> Result<rhai::Array, Box<EvalAltResult>> {
            let (engine, ast) = r
                .exec()
                .ok_or_else(|| -> Box<EvalAltResult> { "parallel: engine not ready".into() })?;
            let mut fns: Vec<FnPtr> = tasks
                .into_iter()
                .map(|d| {
                    d.try_cast::<FnPtr>().ok_or_else(|| -> Box<EvalAltResult> {
                        "parallel: each task must be a zero-arg closure (`|| …`)".into()
                    })
                })
                .collect::<Result<_, _>>()?;

            // Detach each task's captured environment: Rhai closures capture outer
            // variables as *shared* values (stored as curry), so two tasks touching
            // the same one (e.g. `ctx`) would trip Rhai's data-race guard. Flatten
            // each capture into an independent copy — agent/HTTP handles still point
            // at the same Rust-side state (their `Arc`s are cloned), so the verbs
            // work, but the Rhai values no longer alias across threads.
            for f in &mut fns {
                for v in f.iter_curry_mut() {
                    *v = v.flatten_clone();
                }
            }

            let mut handles = Vec::with_capacity(fns.len());
            for f in fns {
                let (e, a) = (engine.clone(), ast.clone());
                handles.push(std::thread::spawn(move || {
                    f.call::<Dynamic>(&e, &a, ()).map_err(|err| err.to_string())
                }));
            }
            // Wait for ALL (so every task's assertions are emitted), then surface
            // the first failure.
            let mut out = rhai::Array::new();
            let mut first_err: Option<String> = None;
            for h in handles {
                match h.join() {
                    Ok(Ok(v)) => out.push(v),
                    Ok(Err(e)) => {
                        first_err.get_or_insert(e);
                    }
                    Err(_) => {
                        first_err.get_or_insert_with(|| "parallel: a task panicked".to_string());
                    }
                }
            }
            match first_err {
                Some(e) => Err(e.into()),
                None => Ok(out),
            }
        }
    );

    // Suite structure: register `scenario`/`setup`/`teardown` into the registry,
    // which the host reads after the top-level pass.
    let r = registry.clone();
    reg!(
        engine,
        "scenario",
        ["name: string", "body: Fn", "()"],
        "/// Register a named scenario, run in isolation (fresh agents, torn down\n\
         /// after). The body may take the `setup()` context: `|ctx| { … }`.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// scenario(\"answered call\", |ctx| {\n\
         ///     ctx.caller.dial(ctx.callee);\n\
         ///     await_until(|| assert(ctx.callee.state).equals(State::Ringing), \"15s\");\n\
         ///     ctx.callee.accept();\n\
         /// });\n\
         /// ```",
        move |name: &str, body: FnPtr| r.add_scenario(
            ScenarioInfo {
                name: name.to_string(),
                ..Default::default()
            },
            body
        )
    );
    let r = registry.clone();
    reg!(
        engine,
        "scenario",
        ["name: string", "options: map", "body: Fn", "()"],
        "/// Register a scenario with options `#{ tags: [\"smoke\"], skip: true|\"reason\",\n\
         /// only: true }`. `--tag`/`--exclude-tag` filter by tag; a skipped scenario is\n\
         /// reported but not run; if any scenario sets `only`, only those run.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// scenario(\"smoke: answered\", #{ tags: [\"smoke\"] }, |ctx| {\n\
         ///     ctx.caller.dial(ctx.callee);\n\
         ///     ctx.callee.accept();\n\
         /// });\n\
         /// ```",
        move |name: &str, options: Map, body: FnPtr| -> Result<(), Box<EvalAltResult>> {
            let info = convert::scenario_info_from_map(name, &options)?;
            r.add_scenario(info, body);
            Ok(())
        }
    );
    let r = registry.clone();
    reg!(
        engine,
        "setup",
        ["body: Fn", "()"],
        "/// Run before each scenario; its return value is passed to the scenario\n\
         /// (and teardown) as `ctx`. Typically creates and registers the agents.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// setup(|| {\n\
         ///     let caller = agent(\"Caller\", #{ username: env(\"A_USER\"), domain: env(\"SIP_DOMAIN\"), password: env(\"A_PASS\") });\n\
         ///     caller.register();\n\
         ///     #{ caller: caller }\n\
         /// });\n\
         /// ```",
        move |body: FnPtr| r.set_setup(body)
    );
    let r = registry.clone();
    reg!(
        engine,
        "teardown",
        ["body: Fn", "()"],
        "/// Run after each scenario (even on failure); receives the `setup` context.\n\
         /// # Example\n\
         /// ```rhai\n\
         /// teardown(|ctx| { ctx.caller.hangup(); });\n\
         /// ```",
        move |body: FnPtr| r.set_teardown(body)
    );

    // skip([reason]) — abort the current scenario as *skipped* (reported, not
    // failed). Useful for runtime/env-gated conditions: `if env(...) != ... { skip(...) }`.
    reg!(
        engine,
        "skip",
        ["reason: string", "()"],
        "/// Skip the current scenario at runtime with a reason (reported, not failed).\n\
         /// # Example\n\
         /// ```rhai\n\
         /// if env(\"STAGE\") != \"prod\" { skip(\"prod only\") }\n\
         /// ```",
        |reason: &str| -> Result<(), Box<EvalAltResult>> {
            Err(Box::new(EvalAltResult::ErrorRuntime(
                Dynamic::from(SkipMarker {
                    reason: Some(reason.to_string()),
                }),
                Position::NONE,
            )))
        }
    );
    reg!(
        engine,
        "skip",
        ["()"],
        "/// Skip the current scenario at runtime (reported, not failed).",
        || -> Result<(), Box<EvalAltResult>> {
            Err(Box::new(EvalAltResult::ErrorRuntime(
                Dynamic::from(SkipMarker { reason: None }),
                Position::NONE,
            )))
        }
    );
}
