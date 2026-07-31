//! The mock HTTP server DSL: the `MockServer` and `PathMatch` JS classes, the
//! `mockServer`/`regex`/`jsonResponse`/`textResponse` globals, and the request/
//! response marshalling plus the `pump_bridged` bridge that runs dynamic responder
//! closures on the scenario thread.

use super::super::convert::throw;
use super::super::host::Registry;
use super::super::tsgen::declare;
use super::core::HostState;
use crate::engine::mock_server::{
    self, BridgedRequest, MockRequest, MockResponse, MockServerInner, PathMatcher, Responder,
};
use indexmap::IndexMap;
use rquickjs::class::Trace;
use rquickjs::function::Opt;
use rquickjs::{
    Class, Ctx as JsCtx, Function, IntoJs, JsLifetime, Object, Persistent, Result as JsResult,
    Value,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

// ── Mock-server-domain TS types ──
/// The response spec read by `respond`/`jsonResponse`/`textResponse`. A single
/// source of truth: the `#[derive(Interface)]` exports the `MockResponseSpec`
/// interface and a `TS_FIELDS` list. The fields are parsed off a raw object in
/// [`mock_response`] (not read off this struct), so it is a shape-only marker.
#[allow(dead_code)] // a codegen source: only its name + interface are used.
#[derive(ringo_flow_macros::TsInterface)]
struct MockResponseSpec {
    /// HTTP status code to return (default `200`).
    status: Option<u16>,
    /// Response body (a string; use `jsonResponse`/`textResponse` for shorthands).
    body: Option<String>,
    /// `Content-Type` header to set.
    #[jsdoc(rename = "contentType")]
    content_type: Option<String>,
    /// Extra response headers.
    headers: Option<IndexMap<String, String>>,
}

// `MockResponder` has no rquickjs binding to derive from → declared verbatim (the doc
// is part of the emitted TS, for the editor hover).
declare!(
    r#"/** A static response, or a closure invoked per request (runs on the scenario
 *  thread, pumped from `until`, so it may close over scenario state). */
type MockResponder = MockResponseSpec | ((req: MockRequestInfo) => MockResponseSpec);"#
);

/// A received request, exposed to scenarios via `lastRequest`/`requests` and to
/// dynamic responder closures. Built in Rust and `into_js`-converted so the
/// interface and the produced object share one source.
#[derive(rquickjs::IntoJs, ringo_flow_macros::TsInterface)]
struct MockRequestInfo {
    /// HTTP method (`GET`, `POST`, …).
    method: String,
    /// Request path (without query string).
    path: String,
    /// Parsed query-string parameters.
    query: HashMap<String, String>,
    /// Request headers.
    headers: HashMap<String, String>,
    /// Raw request body.
    body: String,
}

/// A running mock HTTP server. `respond(...)` takes either a static response
/// object or a JS closure: the closure runs on the scenario thread (pumped from
/// `until`), so it can compute the response dynamically and close over
/// scenario state. See the `Responder::Bridged` channel in the engine.
#[derive(Trace, JsLifetime, Clone)]
#[rquickjs::class(rename = "MockServer")]
pub struct MockServer {
    #[qjs(skip_trace)]
    pub inner: Arc<MockServerInner>,
    /// To register dynamic (`Bridged`) responders, whose JS closures the scenario
    /// thread pumps.
    #[qjs(skip_trace)]
    pub registry: Arc<Registry>,
}

#[ringo_flow_macros::ts_export]
#[rquickjs::methods]
impl MockServer {
    /// Start a mock HTTP server: `new MockServer()` (free port) or
    /// `new MockServer({ port })`.
    #[qjs(constructor)]
    fn new<'js>(
        cx: JsCtx<'js>,
        #[jsdoc(type = "{ port?: number }")] opts: Opt<Object<'js>>,
    ) -> JsResult<MockServer> {
        let (eng, reg) = {
            let h = cx
                .userdata::<HostState>()
                .expect("host state stored at install");
            (h.eng.clone(), h.reg.clone())
        };
        let port = match &opts.0 {
            Some(o) => {
                super::super::bindings::reject_unknown_keys("MockServer", o, &["port"])
                    .map_err(|e| throw(&cx, &e))?;
                o.get::<_, Option<u16>>("port").ok().flatten()
            }
            None => None,
        };
        let inner = mock_server::start(&eng, port).map_err(|e| throw(&cx, &e))?;
        eng.register_mock(inner.clone());
        Ok(MockServer {
            inner,
            registry: reg,
        })
    }

    /// The server's base URL (`http://127.0.0.1:<port>`), to point the SUT at.
    #[qjs(get)]
    fn url(&self) -> String {
        self.inner.url()
    }
    #[qjs(get)]
    fn port(&self) -> i64 {
        self.inner.port() as i64
    }
    /// Register a route: a static response object, or a per-request closure (runs on
    /// the scenario thread, pumped from `until`). `path` is a string or
    /// `regex(...)`; a leading method arg is optional.
    #[jsdoc(
        sig = "respond(method: string, path: string | PathMatch, response: MockResponder): void"
    )]
    #[jsdoc(sig = "respond(path: string | PathMatch, response: MockResponder): void")]
    fn respond<'js>(
        &self,
        ctx: JsCtx<'js>,
        a: Value<'js>,
        b: Value<'js>,
        c: Opt<Value<'js>>,
    ) -> JsResult<()> {
        // (method, path, response) when a third arg is given; else (path, response).
        let (method, path_val, resp) = match c.0 {
            Some(resp) => {
                let method = a
                    .as_string()
                    .and_then(|s| s.to_string().ok())
                    .ok_or_else(|| {
                        throw(
                            &ctx,
                            "respond(method, path, response): method must be a string",
                        )
                    })?;
                (Some(method), b, resp)
            }
            None => (None, a, b),
        };
        let matcher = path_matcher(&ctx, &path_val)?;
        let responder = if let Some(f) = resp.as_function() {
            // Dynamic responder: persist the closure and hand the engine a channel.
            // Each request is bridged back to the scenario thread (see `pump_bridged`),
            // where the closure runs in its real JS context with its captures intact.
            let (tx, rx) = mpsc::channel::<BridgedRequest>(16);
            let saved = Persistent::save(&ctx, f.clone());
            self.registry.add_bridged(rx, saved);
            Responder::Bridged(tx)
        } else if let Some(obj) = resp.as_object() {
            Responder::Static(mock_response(obj))
        } else {
            return Err(throw(
                &ctx,
                "respond: response must be an object or a function",
            ));
        };
        self.inner.set_route(method, matcher, responder);
        Ok(())
    }
    /// How many requests arrived on `path` (string or `regex(...)`, any method) —
    /// poll via `until`.
    #[qjs(rename = "requestCount")]
    fn request_count<'js>(
        &self,
        ctx: JsCtx<'js>,
        #[jsdoc(type = "string | PathMatch")] path: Value<'js>,
    ) -> JsResult<i64> {
        Ok(self.inner.request_count(&path_matcher(&ctx, &path)?))
    }
    /// The most recent request on `path` (string or `regex(...)`) as
    /// `{ method, path, query, headers, body }`, or `undefined`.
    #[qjs(rename = "lastRequest")]
    #[jsdoc(type = "MockRequestInfo | undefined")]
    fn last_request<'js>(
        &self,
        ctx: JsCtx<'js>,
        #[jsdoc(type = "string | PathMatch")] path: Value<'js>,
    ) -> JsResult<Value<'js>> {
        match self.inner.last_request(&path_matcher(&ctx, &path)?) {
            Some(req) => request_object(&ctx, &req),
            None => Ok(Value::new_undefined(ctx.clone())),
        }
    }
    /// All requests on `path` (string or `regex(...)`), in arrival order.
    #[jsdoc(type = "MockRequestInfo[]")]
    fn requests<'js>(
        &self,
        ctx: JsCtx<'js>,
        #[jsdoc(type = "string | PathMatch")] path: Value<'js>,
    ) -> JsResult<Vec<Value<'js>>> {
        self.inner
            .requests(&path_matcher(&ctx, &path)?)
            .iter()
            .map(|req| request_object(&ctx, req))
            .collect()
    }
    /// Stop the server early (it otherwise stops at scenario teardown).
    fn stop(&self) {
        self.inner.shutdown();
    }
}

/// A path matcher passed to `respond`/`requestCount`/`lastRequest`/`requests`:
/// either an exact path (a plain string) or a regex built with `regex(...)`.
#[derive(Trace, JsLifetime, Clone)]
#[rquickjs::class(rename = "PathMatch")]
pub struct PathMatch {
    #[qjs(skip_trace)]
    pub inner: PathMatcher,
}

// Opaque matcher handle (built with `regex(...)`); the branded `never` field makes it
// nominal so only a real `regex(...)` result satisfies a `PathMatch` parameter.
declare!(
    r#"/** A regex path matcher built with `regex(...)`, for the mock server's path args. */
interface PathMatch { readonly __pathMatch?: never; }"#
);

/// A regex path matcher for the mock server's respond/requestCount/lastRequest/requests.
#[ringo_flow_macros::ts_global(name = "regex")]
pub(in crate::script::js) fn regex_global<'js>(
    cx: JsCtx<'js>,
    pattern: String,
) -> rquickjs::Result<Class<'js, PathMatch>> {
    let inner = mock_server::PathMatcher::regex(&pattern).map_err(|e| throw(&cx, &e))?;
    Class::instance(cx, PathMatch { inner })
}

/// A `application/json` response spec (body JSON-encoded) for `respond`.
#[ringo_flow_macros::ts_global(name = "jsonResponse")]
#[jsdoc(type = "MockResponseSpec")]
pub(in crate::script::js) fn json_response_global<'js>(
    cx: JsCtx<'js>,
    body: Value<'js>,
    status: Opt<i64>,
) -> rquickjs::Result<Object<'js>> {
    let json = cx
        .json_stringify(body)?
        .and_then(|s| s.to_string().ok())
        .unwrap_or_else(|| "null".to_string());
    let o = Object::new(cx.clone())?;
    o.set("status", status.0.unwrap_or(200))?;
    o.set("body", json)?;
    o.set("contentType", "application/json")?;
    Ok(o)
}

/// A `text/plain` response spec for `respond`.
#[ringo_flow_macros::ts_global(name = "textResponse")]
#[jsdoc(type = "MockResponseSpec")]
pub(in crate::script::js) fn text_response_global<'js>(
    cx: JsCtx<'js>,
    body: String,
    status: Opt<i64>,
) -> rquickjs::Result<Object<'js>> {
    let o = Object::new(cx.clone())?;
    o.set("status", status.0.unwrap_or(200))?;
    o.set("body", body)?;
    o.set("contentType", "text/plain")?;
    Ok(o)
}

/// Resolve a JS path argument to a [`PathMatcher`]: a string is an exact path, a
/// `PathMatch` (from `regex(...)`) carries a compiled regex.
fn path_matcher<'js>(ctx: &JsCtx<'js>, v: &Value<'js>) -> JsResult<PathMatcher> {
    if let Some(s) = v.as_string() {
        return Ok(PathMatcher::Exact(s.to_string()?));
    }
    if let Some(obj) = v.as_object() {
        if let Some(pm) = Class::<PathMatch>::from_object(obj) {
            return Ok(pm.borrow().inner.clone());
        }
    }
    Err(throw(ctx, "path must be a string or a regex(...) matcher"))
}

/// A received [`MockRequest`] → a JS object `{ method, path, query, headers, body }`,
/// built as a [`MockRequestInfo`] and `into_js`-ed so the shape matches the interface.
fn request_object<'js>(ctx: &JsCtx<'js>, req: &MockRequest) -> JsResult<Value<'js>> {
    MockRequestInfo {
        method: req.method.clone(),
        path: req.path.clone(),
        query: req.query.clone(),
        headers: req.headers.clone(),
        body: req.body.clone(),
    }
    .into_js(ctx)
}

/// Build a [`MockResponse`] from a JS `{ status, body, contentType, headers }`
/// object; all fields optional (default 200 / empty body).
fn mock_response(obj: &Object<'_>) -> MockResponse {
    let headers = obj
        .get::<_, Option<Object>>("headers")
        .ok()
        .flatten()
        .map(|h| {
            h.props::<String, String>()
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    MockResponse {
        status: obj
            .get::<_, Option<u16>>("status")
            .ok()
            .flatten()
            .unwrap_or(200),
        content_type: obj.get::<_, Option<String>>("contentType").ok().flatten(),
        body: obj
            .get::<_, Option<String>>("body")
            .ok()
            .flatten()
            .unwrap_or_default(),
        headers,
    }
}

/// Drain every pending request for the dynamic (`Bridged`) mock responders and
/// answer each by invoking its persisted JS closure, then replying on the one-shot
/// the HTTP handler is awaiting. Called from `until`'s poll loop — this runs on
/// the scenario thread, the only place a `!Send` QuickJS closure may execute.
pub(in crate::script::js) fn pump_bridged<'js>(ctx: &JsCtx<'js>, registry: &Registry) {
    let mut bridged = registry.bridged.lock().unwrap();
    for (rx, closure) in bridged.iter_mut() {
        while let Ok((req, resp_tx)) = rx.try_recv() {
            let _ = resp_tx.send(call_bridged_responder(ctx, closure, req));
        }
    }
}

/// Marshal a [`MockRequest`] into a JS object, call the responder closure, and turn
/// its return value into a [`MockResponse`]. A throw or a non-object return becomes a
/// `500` (the failure is the scenario's bug, not the API's, and isn't leaked over
/// HTTP — mirroring the engine's `Dynamic` responder handling).
fn call_bridged_responder(
    ctx: &JsCtx<'_>,
    closure: &Persistent<Function<'static>>,
    req: MockRequest,
) -> MockResponse {
    let built = (|| -> JsResult<Option<MockResponse>> {
        let f: Function = closure.clone().restore(ctx)?;
        let obj = request_object(ctx, &req)?;
        let ret: Value = f.call((obj,))?;
        Ok(ret.as_object().map(mock_response))
    })();
    match built {
        Ok(Some(resp)) => resp,
        other => {
            // A throw leaves a pending exception on the context; clear it so it can't
            // leak into the next `until` poll. (`catch` is a no-op if none.)
            if other.is_err() {
                let _ = ctx.catch();
            }
            MockResponse {
                status: 500,
                content_type: None,
                headers: Vec::new(),
                body: String::new(),
            }
        }
    }
}
