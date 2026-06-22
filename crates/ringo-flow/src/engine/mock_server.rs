//! Language-neutral HTTP mock server: a scenario starts one, registers per-route
//! responses (static, or a dynamic responder supplied by the language adapter),
//! and inspects the requests it received. It backs the webhook-driven e2e pattern
//! — a telephony API calls our webhook and we answer with the actions it should
//! perform (Twilio-style).
//!
//! The server runs on the shared tokio runtime (`Ctx::rt`); the script thread only
//! ever touches the route table and the recorded requests behind a `Mutex`, so
//! waiting for a webhook is just polling `request_count(...)` via `await_until` —
//! no second blocking mechanism.

use super::ctx::Ctx;
use crate::runtime::report::Event;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, Response, StatusCode, header};
use axum::{Router, routing::any};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Largest request body the mock will buffer (webhooks are small JSON payloads).
const MAX_BODY: usize = 1 << 20; // 1 MiB

/// A request the mock server received, exposed to scenarios for assertions.
#[derive(Clone, Debug)]
pub struct MockRequest {
    pub method: String,
    pub path: String,
    /// Query parameters (percent-decoded), last value wins on duplicates.
    pub query: HashMap<String, String>,
    /// Request headers, names lower-cased (like [`super::http::HttpResponse`]).
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// The response the mock returns for a matched route.
#[derive(Clone)]
pub struct MockResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// What to answer for a route: a fixed response, or a closure the language adapter
/// supplies (e.g. a Rhai responder). The closure must be pure — request in,
/// response out — and must not touch agent sessions (it runs on a runtime worker,
/// concurrently with the script thread).
pub enum Responder {
    Static(MockResponse),
    /// A responder closure. On `Err`, the failure is logged to the scenario (never
    /// exposed over HTTP); the caller gets a bare `500`.
    Dynamic(Box<dyn Fn(MockRequest) -> Result<MockResponse, String> + Send + Sync>),
}

type RouteKey = (String, String); // (UPPERCASE method, exact path)

/// The shared state of a running mock server. Cheap to clone via `Arc`; the script
/// handle, the serving task and the [`Ctx`] teardown list all hold one.
pub struct MockServerInner {
    port: u16,
    routes: Mutex<HashMap<RouteKey, Arc<Responder>>>,
    recorded: Mutex<Vec<MockRequest>>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

impl MockServerInner {
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The base URL to hand to the system under test, e.g. `http://127.0.0.1:8080`.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Register (or replace) the response for `method path`. Re-registering between
    /// webhooks is how a deterministic flow stages successive answers.
    pub fn set_route(&self, method: &str, path: &str, responder: Responder) {
        self.routes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                (method.to_uppercase(), path.to_string()),
                Arc::new(responder),
            );
    }

    /// Number of requests received on `path` (any method) — pollable in `await_until`.
    pub fn request_count(&self, path: &str) -> i64 {
        self.recorded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|r| r.path == path)
            .count() as i64
    }

    /// The most recent request received on `path`, or `None`.
    pub fn last_request(&self, path: &str) -> Option<MockRequest> {
        self.recorded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .rev()
            .find(|r| r.path == path)
            .cloned()
    }

    /// All requests received on `path`, in arrival order.
    pub fn requests(&self, path: &str) -> Vec<MockRequest> {
        self.recorded
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|r| r.path == path)
            .cloned()
            .collect()
    }

    /// Stop serving (idempotent). Called by [`Ctx::reset_sessions`] at teardown and
    /// by the optional `stop()` verb.
    pub fn shutdown(&self) {
        if let Some(tx) = self
            .shutdown
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = tx.send(());
        }
    }
}

/// State shared with the serving task: the server's tables plus the context to
/// report received requests through. Holds the context by `Weak` so the
/// `Ctx → mock_servers → task → Ctx` chain can't form a reference cycle.
#[derive(Clone)]
struct Handler {
    inner: Arc<MockServerInner>,
    ctx: std::sync::Weak<Ctx>,
}

/// Start a mock server. `port` is the requested port, or `None` to let the OS pick
/// a free one (the chosen port is then read back via [`MockServerInner::url`]). The
/// server runs on `ctx.rt`; the returned handle is registered for teardown by the
/// caller.
pub fn start(ctx: &Arc<Ctx>, port: Option<u16>) -> Result<Arc<MockServerInner>, String> {
    // Bind synchronously so a port clash is a clear error at start time, then hand
    // the listener to tokio.
    let std_listener = std::net::TcpListener::bind(("127.0.0.1", port.unwrap_or(0)))
        .map_err(|e| format!("mock_server: bind 127.0.0.1:{}: {e}", port.unwrap_or(0)))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| format!("mock_server: set_nonblocking: {e}"))?;
    let bound_port = std_listener
        .local_addr()
        .map_err(|e| format!("mock_server: local_addr: {e}"))?
        .port();

    let (tx, rx) = oneshot::channel::<()>();
    let inner = Arc::new(MockServerInner {
        port: bound_port,
        routes: Mutex::new(HashMap::new()),
        recorded: Mutex::new(Vec::new()),
        shutdown: Mutex::new(Some(tx)),
    });

    let handler = Handler {
        inner: inner.clone(),
        ctx: Arc::downgrade(ctx),
    };
    ctx.rt.spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("mock_server: listener: {e}");
                return;
            }
        };
        let app = Router::new().fallback(any(serve)).with_state(handler);
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await;
    });

    Ok(inner)
}

/// The single catch-all handler: record the request, report it, look up a route
/// and run its responder (404 if none matches).
async fn serve(State(state): State<Handler>, req: Request<Body>) -> Response<Body> {
    let (parts, body) = req.into_parts();
    let body = to_bytes(body, MAX_BODY)
        .await
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let method = parts.method.as_str().to_uppercase();
    let path = parts.uri.path().to_string();
    let query = parse_query(parts.uri.query().unwrap_or(""));
    let headers = parts
        .headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();

    let mreq = MockRequest {
        method: method.clone(),
        path: path.clone(),
        query,
        headers,
        body,
    };

    // Record before responding so a synchronous responder still sees it counted.
    state
        .inner
        .recorded
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(mreq.clone());

    let responder = state
        .inner
        .routes
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&(method.clone(), path.clone()))
        .cloned();

    if let Some(ctx) = state.ctx.upgrade() {
        ctx.emit(&Event::MockRequest {
            method: &method,
            path: &path,
            matched: responder.is_some(),
        });
    }

    let resp = match responder {
        Some(r) => match &*r {
            Responder::Static(resp) => resp.clone(),
            // A responder failure is the scenario's bug, not the API's: log it and
            // answer with a bare 500 so the error text isn't leaked over HTTP.
            Responder::Dynamic(f) => f(mreq).unwrap_or_else(|error| {
                if let Some(ctx) = state.ctx.upgrade() {
                    ctx.emit(&Event::MockError {
                        method: &method,
                        path: &path,
                        error: &error,
                    });
                }
                MockResponse {
                    status: 500,
                    content_type: None,
                    headers: Vec::new(),
                    body: String::new(),
                }
            }),
        },
        None => MockResponse {
            status: 404,
            content_type: Some("text/plain".into()),
            headers: Vec::new(),
            body: format!("no mock route for {method} {path}"),
        },
    };
    build_response(resp)
}

/// Turn a [`MockResponse`] into an HTTP response, degrading gracefully if a header
/// name/value is invalid rather than panicking the handler.
fn build_response(r: MockResponse) -> Response<Body> {
    let mut builder = Response::builder()
        .status(StatusCode::from_u16(r.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));
    if let Some(ct) = &r.content_type {
        builder = builder.header(header::CONTENT_TYPE, ct);
    }
    for (k, v) in &r.headers {
        builder = builder.header(k.as_str(), v.as_str());
    }
    builder
        .body(Body::from(r.body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Parse a URL query string into a map (last value wins), percent-decoding keys and
/// values and treating `+` as a space.
fn parse_query(q: &str) -> HashMap<String, String> {
    q.split('&')
        .filter(|s| !s.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => (percent_decode(k), percent_decode(v)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

/// Minimal `application/x-www-form-urlencoded` decode: `+` → space, `%XX` → byte.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push((hi * 16 + lo) as u8);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_query_decodes_and_splits() {
        let q = parse_query("event=incoming+call&id=%2B49&flag");
        assert_eq!(q.get("event").unwrap(), "incoming call");
        assert_eq!(q.get("id").unwrap(), "+49");
        assert_eq!(q.get("flag").unwrap(), "");
        assert!(parse_query("").is_empty());
    }

    /// A reporter that drops events (the test only cares about HTTP behaviour).
    struct Silent;
    impl crate::runtime::report::Reporter for Silent {
        fn emit(&mut self, _: &crate::runtime::report::Event) {}
    }

    fn test_ctx() -> (tokio::runtime::Runtime, Arc<Ctx>) {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let ctx = Arc::new(Ctx::new(
            rt.handle().clone(),
            Box::new(Silent),
            std::time::Duration::from_secs(5),
        ));
        (rt, ctx)
    }

    #[test]
    fn static_dynamic_and_unmatched_routes() {
        let (_rt, ctx) = test_ctx();
        let server = start(&ctx, None).unwrap();
        server.set_route(
            "POST",
            "/voice",
            Responder::Static(MockResponse {
                status: 200,
                content_type: Some("application/json".into()),
                headers: vec![("X-Mock".into(), "1".into())],
                body: r#"{"ok":true}"#.into(),
            }),
        );
        // A dynamic responder echoes the request body back with a 201.
        server.set_route(
            "POST",
            "/echo",
            Responder::Dynamic(Box::new(|req| {
                Ok(MockResponse {
                    status: 201,
                    content_type: Some("text/plain".into()),
                    headers: vec![],
                    body: req.body,
                })
            })),
        );
        // A failing responder must yield a bare 500 — no error text over HTTP.
        server.set_route(
            "POST",
            "/boom",
            Responder::Dynamic(Box::new(|_| Err("secret internal detail".into()))),
        );

        let client = reqwest::blocking::Client::new();

        let r = client
            .post(format!("{}/voice", server.url()))
            .header("X-Test", "abc")
            .body(r#"{"event":"incoming_call"}"#)
            .send()
            .unwrap();
        assert_eq!(r.status().as_u16(), 200);
        assert_eq!(r.headers()["x-mock"], "1");
        assert_eq!(r.text().unwrap(), r#"{"ok":true}"#);

        let r = client
            .post(format!("{}/echo", server.url()))
            .body("ping")
            .send()
            .unwrap();
        assert_eq!(r.status().as_u16(), 201);
        assert_eq!(r.text().unwrap(), "ping");

        // No route → 404.
        let r = client
            .get(format!("{}/missing", server.url()))
            .send()
            .unwrap();
        assert_eq!(r.status().as_u16(), 404);

        // Failing responder → bare 500, error text not leaked to the caller.
        let r = client
            .post(format!("{}/boom", server.url()))
            .send()
            .unwrap();
        assert_eq!(r.status().as_u16(), 500);
        assert!(r.text().unwrap().is_empty(), "500 body must be empty");

        // Recording: the two POSTs are captured per path; headers are lower-cased.
        assert_eq!(server.request_count("/voice"), 1);
        assert_eq!(server.request_count("/echo"), 1);
        let req = server.last_request("/voice").unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.headers.get("x-test").map(String::as_str), Some("abc"));
        assert_eq!(req.body, r#"{"event":"incoming_call"}"#);

        server.shutdown();
    }
}
