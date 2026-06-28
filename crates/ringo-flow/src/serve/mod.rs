//! `ringo-flow serve`: a long-lived monitor that runs scenario files on a cron
//! schedule (and on demand via HTTP) and exposes the results as Prometheus
//! metrics.
//!
//! This module is the orchestration: it parses the config, starts the worker
//! and cron schedulers, and serves the HTTP API ([`api`]). Each run is a fresh
//! `ringo-flow run --json --metrics` subprocess ([`runner`]) — the baresip FFI
//! can't be re-initialised in-process, and a child gives crash isolation + a
//! hard timeout. Runs are serialised through a single worker: there's one global
//! backend per process, so two at once would collide. The cron schedulers and
//! `POST /run` both feed that one queue.

mod api;
mod config;
mod logging;
mod metrics;
mod prometheus;
mod runner;

pub use config::{Config, Overrides};
pub use logging::init as init_logging;

use tracing::{error, info, warn};

use anyhow::{Context, Result};
use api::{AppState, RunRequest};
use metrics::MetricsStore;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// Run the monitor server: parse the config, apply CLI/env overrides, start the
/// worker + cron schedulers, and serve HTTP until killed. Its tokio runtime is
/// built by the caller in `main`.
pub async fn serve(config_path: &std::path::Path, overrides: Overrides) -> Result<()> {
    let mut config = Config::load(config_path)?;
    config.apply_overrides(&overrides)?;
    let binary = match &config.binary {
        Some(b) => b.clone(),
        None => std::env::current_exe().context("locate the running ringo-flow binary")?,
    };
    let default_timeout = config.default_timeout()?;
    let listen = config.listen.clone();
    let config = Arc::new(config);
    let store = Arc::new(Mutex::new(MetricsStore::default()));

    // The single serialised run queue.
    let (queue_tx, queue_rx) = mpsc::channel::<RunRequest>(64);
    tokio::spawn(worker(
        queue_rx,
        binary,
        Arc::clone(&config),
        default_timeout,
        Arc::clone(&store),
    ));

    // One scheduler task per scheduled monitor, unless schedulers are disabled.
    let mut scheduled = 0;
    if config.scheduler {
        for m in &config.monitors {
            if let Some(expr) = &m.schedule {
                scheduled += 1;
                tokio::spawn(scheduler(m.name.clone(), expr.clone(), queue_tx.clone()));
            }
        }
    }
    info!(
        monitors = config.monitors.len(),
        scheduled,
        schedulers = config.scheduler,
        metrics = config.metrics.enabled,
        listen = %listen,
        "ringo-flow serve started"
    );

    let state = AppState {
        config: Arc::clone(&config),
        store,
        queue: queue_tx,
    };
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .with_context(|| format!("bind {listen}"))?;
    axum::serve(listener, api::router(state))
        .await
        .context("http server")?;
    Ok(())
}

/// Drain the queue one run at a time: look up the monitor, run it, record the
/// result, and reply if someone is waiting.
async fn worker(
    mut rx: mpsc::Receiver<RunRequest>,
    binary: PathBuf,
    config: Arc<Config>,
    default_timeout: Duration,
    store: Arc<Mutex<MetricsStore>>,
) {
    while let Some(req) = rx.recv().await {
        // The HTTP handler validates the name before queuing, so a miss here only
        // happens for a scheduled job whose config changed — skip it cleanly.
        let Some(m) = config.monitors.iter().find(|m| m.name == req.monitor) else {
            continue;
        };
        let timeout = m.timeout(default_timeout);
        info!(monitor = %m.name, "running monitor");
        let outcome = runner::run(&binary, m, timeout).await;
        let now_unix = chrono::Utc::now().timestamp();
        store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .record(&m.name, &outcome, now_unix);
        let duration_ms = outcome.duration.as_millis() as u64;
        let scenarios = outcome.scenarios.len();
        if outcome.passed {
            info!(monitor = %m.name, duration_ms, scenarios, "monitor run passed");
        } else {
            warn!(
                monitor = %m.name,
                duration_ms,
                error = outcome.error.as_deref().unwrap_or(""),
                "monitor run failed"
            );
        }
        if let Some(respond) = req.respond {
            let _ = respond.send(api::summarize(&m.name, &outcome));
        }
    }
}

/// Sleep until each cron occurrence, then enqueue a fire-and-forget run.
async fn scheduler(name: String, expr: String, queue: mpsc::Sender<RunRequest>) {
    let cron = match expr.parse::<croner::Cron>() {
        Ok(c) => c,
        // Validated at startup, so this shouldn't happen; bail out of the task.
        Err(e) => {
            error!(monitor = %name, cron = %expr, error = %e, "invalid cron, scheduler stopped");
            return;
        }
    };
    loop {
        let now = chrono::Local::now();
        let next = match cron.find_next_occurrence(&now, false) {
            Ok(n) => n,
            Err(e) => {
                warn!(monitor = %name, error = %e, "no next cron occurrence, scheduler stopped");
                return;
            }
        };
        let wait = (next - now).to_std().unwrap_or(Duration::from_secs(1));
        tokio::time::sleep(wait).await;
        if queue
            .send(RunRequest {
                monitor: name.clone(),
                respond: None,
            })
            .await
            .is_err()
        {
            return; // server shutting down
        }
    }
}
