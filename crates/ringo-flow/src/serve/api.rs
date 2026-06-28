//! The HTTP surface: the axum router, its handlers, the JSON response types and
//! the run-request queue item. The orchestration (worker, schedulers) lives in
//! the parent module and drives runs through [`RunRequest`].

use super::config::Config;
use super::metrics::MetricsStore;
use super::prometheus;
use super::runner::RunOutcome;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

/// A queued run: a monitor name, with an optional channel to reply on (set for
/// `POST /run`, `None` for scheduled fire-and-forget runs).
pub(super) struct RunRequest {
    pub(super) monitor: String,
    pub(super) respond: Option<oneshot::Sender<RunSummary>>,
}

/// The JSON summary returned by `POST /run` and logged after each run.
#[derive(Serialize, Clone)]
pub(super) struct RunSummary {
    pub(super) monitor: String,
    pub(super) passed: bool,
    pub(super) timed_out: bool,
    pub(super) duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
    /// The scenarios the run executed, each with its agents. Only read here (for
    /// the JSON response); the worker uses the fields above for its log line.
    scenarios: Vec<ScenarioSummary>,
}

#[derive(Serialize, Clone)]
struct ScenarioSummary {
    name: String,
    passed: bool,
    agents: Vec<AgentSummary>,
}

#[derive(Serialize, Clone)]
struct AgentSummary {
    agent: String,
    registered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    mos: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jitter_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    packet_loss_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rtt_ms: Option<f64>,
}

/// Shared HTTP handler state.
#[derive(Clone)]
pub(super) struct AppState {
    pub(super) config: Arc<Config>,
    pub(super) store: Arc<Mutex<MetricsStore>>,
    pub(super) queue: mpsc::Sender<RunRequest>,
}

/// Build the router. `/metrics` is only mounted when `[metrics].enabled` is set
/// (the default).
pub(super) fn router(state: AppState) -> Router {
    let mut router = Router::new()
        .route("/healthz", get(|| async { "ok\n" }))
        .route("/monitors", get(monitors_handler))
        .route("/run/{name}", post(run_handler));
    if state.config.metrics.enabled {
        router = router.route("/metrics", get(metrics_handler));
    }
    router.with_state(state)
}

/// Build the per-run summary from an outcome (for the HTTP reply + the log line).
pub(super) fn summarize(monitor: &str, outcome: &RunOutcome) -> RunSummary {
    RunSummary {
        monitor: monitor.to_string(),
        passed: outcome.passed,
        timed_out: outcome.timed_out,
        duration_ms: outcome.duration.as_millis(),
        error: outcome.error.clone(),
        scenarios: outcome
            .scenarios
            .iter()
            .map(|s| ScenarioSummary {
                name: s.name.clone(),
                passed: s.passed,
                agents: s
                    .agents
                    .iter()
                    .map(|a| AgentSummary {
                        agent: a.agent.clone(),
                        registered: a.registered,
                        mos: a.mos,
                        jitter_ms: a.jitter_ms,
                        packet_loss_pct: a.packet_loss_pct,
                        rtt_ms: a.rtt_ms,
                    })
                    .collect(),
            })
            .collect(),
    }
}

async fn metrics_handler(State(st): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = &st.config.metrics.bearer_token {
        if !bearer_ok(&headers, token) {
            return (StatusCode::UNAUTHORIZED, "unauthorized\n").into_response();
        }
    }
    let body = prometheus::render(&st.store.lock().unwrap_or_else(|e| e.into_inner()));
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, prometheus::CONTENT_TYPE)],
        body,
    )
        .into_response()
}

/// Whether the `Authorization` header carries the expected bearer token.
fn bearer_ok(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|got| got == token)
        .unwrap_or(false)
}

async fn monitors_handler(State(st): State<AppState>) -> Response {
    #[derive(Serialize)]
    struct Item<'a> {
        name: &'a str,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        schedule: Option<&'a str>,
    }
    let items: Vec<Item> = st
        .config
        .monitors
        .iter()
        .map(|m| Item {
            name: &m.name,
            path: m.path.to_string_lossy().into_owned(),
            schedule: m.schedule.as_deref(),
        })
        .collect();
    json_response(StatusCode::OK, &items)
}

/// `POST /run/{name}` query: `?async=true` enqueues and returns immediately.
#[derive(Deserialize)]
struct RunParams {
    #[serde(rename = "async", default)]
    run_async: bool,
}

async fn run_handler(
    State(st): State<AppState>,
    Path(name): Path<String>,
    Query(params): Query<RunParams>,
) -> Response {
    if !st.config.monitors.iter().any(|m| m.name == name) {
        return (StatusCode::NOT_FOUND, format!("unknown monitor `{name}`\n")).into_response();
    }

    // Async: enqueue and return 202 without waiting for the result (it lands in
    // /metrics). Like a scheduled run, no reply channel.
    if params.run_async {
        let req = RunRequest {
            monitor: name.clone(),
            respond: None,
        };
        if st.queue.send(req).await.is_err() {
            return (StatusCode::SERVICE_UNAVAILABLE, "server shutting down\n").into_response();
        }
        #[derive(Serialize)]
        struct Queued<'a> {
            monitor: &'a str,
            queued: bool,
        }
        return json_response(
            StatusCode::ACCEPTED,
            &Queued {
                monitor: &name,
                queued: true,
            },
        );
    }

    let (tx, rx) = oneshot::channel();
    let req = RunRequest {
        monitor: name,
        respond: Some(tx),
    };
    if st.queue.send(req).await.is_err() {
        return (StatusCode::SERVICE_UNAVAILABLE, "server shutting down\n").into_response();
    }
    match rx.await {
        Ok(summary) => {
            // 200 if the run passed, 502 if it failed — so a caller can gate on
            // the HTTP status alone.
            let status = if summary.passed {
                StatusCode::OK
            } else {
                StatusCode::BAD_GATEWAY
            };
            json_response(status, &summary)
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "run dropped\n").into_response(),
    }
}

/// Serialize `body` as a JSON HTTP response, or 500 on a serialization error.
fn json_response(status: StatusCode, body: &impl Serialize) -> Response {
    match serde_json::to_string(body) {
        Ok(s) => (status, [(header::CONTENT_TYPE, "application/json")], s).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, value.parse().unwrap());
        h
    }

    #[test]
    fn bearer_ok_matches_exact_token() {
        assert!(bearer_ok(&headers("Bearer s3cret"), "s3cret"));
        assert!(!bearer_ok(&headers("Bearer wrong"), "s3cret"));
        assert!(!bearer_ok(&headers("s3cret"), "s3cret")); // missing scheme
        assert!(!bearer_ok(&HeaderMap::new(), "s3cret")); // no header
    }
}
