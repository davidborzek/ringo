//! Runs one monitor as a child `ringo-flow run --json --metrics` process and
//! folds its NDJSON stream into a [`RunOutcome`] — grouped by the scenarios the
//! file ran, each with its agents.
//!
//! Each run is a fresh process on purpose: the baresip FFI initialises global
//! state once per process and tears it down for good at exit, so reusing it
//! across runs in the long-lived server isn't possible. A subprocess also gives
//! crash isolation (a backend segfault can't take the monitor down) and a hard
//! timeout (kill the child, keep serving).

use super::config::MonitorConfig;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Per-agent media-quality sample parsed from a `metric` NDJSON event.
#[derive(Debug, Clone)]
pub struct AgentMetric {
    pub agent: String,
    pub registered: bool,
    pub mos: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub packet_loss_pct: Option<f64>,
    pub rtt_ms: Option<f64>,
}

/// One scenario the run executed, with its per-agent samples.
#[derive(Debug)]
pub struct ScenarioOutcome {
    pub name: String,
    pub passed: bool,
    pub agents: Vec<AgentMetric>,
}

/// The result of one monitor run, as the server records it.
#[derive(Debug)]
pub struct RunOutcome {
    /// Whether the whole run passed (child exit code 0). False on failure,
    /// timeout or spawn error.
    pub passed: bool,
    /// A failure/timeout/spawn message, if any (for `/run` responses + logs).
    pub error: Option<String>,
    /// Wall-clock duration of the run.
    pub duration: Duration,
    /// Whether the run timed out (and was killed).
    pub timed_out: bool,
    /// The scenarios the run executed, each with its agents.
    pub scenarios: Vec<ScenarioOutcome>,
}

/// The `metric` NDJSON event shape (subset we consume).
#[derive(Deserialize)]
struct MetricEvent {
    scenario: String,
    agent: String,
    #[serde(default)]
    registered: bool,
    mos: Option<f64>,
    jitter_ms: Option<f64>,
    packet_loss_pct: Option<f64>,
    rtt_ms: Option<f64>,
}

/// Build the child command line for a monitor (without spawning) — its own fn so
/// it can be unit-tested.
fn build_args(m: &MonitorConfig) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--json".to_string(),
        "--metrics".to_string(),
        "--no-color".to_string(),
        m.path.to_string_lossy().into_owned(),
    ];
    for env in &m.env_file {
        args.push("--env-file".to_string());
        args.push(env.to_string_lossy().into_owned());
    }
    if let Some(name) = &m.scenario {
        args.push("--scenario".to_string());
        args.push(name.clone());
    }
    for tag in &m.tags {
        args.push("--tag".to_string());
        args.push(tag.clone());
    }
    args
}

/// Accumulator for the NDJSON stream: scenarios in first-seen order, their agent
/// samples, and each scenario's pass/fail (when the file reports it per-scenario).
#[derive(Default)]
struct Collected {
    order: Vec<String>,
    agents: HashMap<String, Vec<AgentMetric>>,
    passed: HashMap<String, bool>,
}

impl Collected {
    fn note(&mut self, scenario: &str) {
        if !self.order.iter().any(|s| s == scenario) {
            self.order.push(scenario.to_string());
        }
    }

    /// Fold one NDJSON line. Unknown events and unparseable lines are ignored
    /// (the child's exit code is the source of truth for the overall pass/fail).
    fn fold_line(&mut self, line: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        match value.get("event").and_then(|e| e.as_str()) {
            Some("metric") => {
                if let Ok(m) = serde_json::from_value::<MetricEvent>(value) {
                    self.note(&m.scenario);
                    self.agents
                        .entry(m.scenario)
                        .or_default()
                        .push(AgentMetric {
                            agent: m.agent,
                            registered: m.registered,
                            mos: m.mos,
                            jitter_ms: m.jitter_ms,
                            packet_loss_pct: m.packet_loss_pct,
                            rtt_ms: m.rtt_ms,
                        });
                }
            }
            Some("scenario_finished") => {
                let name = value.get("name").and_then(|n| n.as_str());
                let passed = value.get("passed").and_then(|p| p.as_bool());
                if let (Some(name), Some(passed)) = (name, passed) {
                    self.note(name);
                    self.passed.insert(name.to_string(), passed);
                }
            }
            _ => {}
        }
    }

    /// Build the ordered scenario outcomes, falling back to `overall` for a
    /// scenario the file didn't report a pass/fail for (e.g. a single-scenario
    /// file, which emits `finished` rather than `scenario_finished`).
    fn into_scenarios(mut self, overall: bool) -> Vec<ScenarioOutcome> {
        self.order
            .iter()
            .map(|name| ScenarioOutcome {
                name: name.clone(),
                passed: self.passed.get(name).copied().unwrap_or(overall),
                agents: self.agents.remove(name).unwrap_or_default(),
            })
            .collect()
    }
}

/// Spawn `binary run --json --metrics …` for `m`, fold its stream into scenario
/// outcomes and pass/fail, killing it if it exceeds `timeout`. Never panics —
/// spawn failures and timeouts come back as a failed [`RunOutcome`].
pub async fn run(binary: &Path, m: &MonitorConfig, timeout: Duration) -> RunOutcome {
    let started = Instant::now();
    let mut child = match Command::new(binary)
        .args(build_args(m))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return RunOutcome {
                passed: false,
                error: Some(format!("spawn {}: {e}", binary.display())),
                duration: started.elapsed(),
                timed_out: false,
                scenarios: Vec::new(),
            };
        }
    };

    // Drain both pipes concurrently so a chatty child can't fill one and stall.
    // stderr carries the failure reason (file-not-found, panics, the human error
    // line) since `--json` puts events on stdout.
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let collector = tokio::spawn(collect(stdout));
    let errors = tokio::spawn(collect_lines(stderr));

    let wait = tokio::time::timeout(timeout, child.wait()).await;
    let collected = collector.await.unwrap_or_default();
    let stderr_tail = errors.await.unwrap_or_default();
    let duration = started.elapsed();

    match wait {
        // Process exited within the timeout.
        Ok(Ok(status)) => RunOutcome {
            passed: status.success(),
            error: (!status.success())
                .then(|| with_reason(&format!("scenario failed (exit {status})"), &stderr_tail)),
            duration,
            timed_out: false,
            scenarios: collected.into_scenarios(status.success()),
        },
        // Waiting on the process itself errored.
        Ok(Err(e)) => RunOutcome {
            passed: false,
            error: Some(format!("wait for child: {e}")),
            duration,
            timed_out: false,
            scenarios: collected.into_scenarios(false),
        },
        // Timed out — kill the child and report it.
        Err(_) => {
            let _ = child.kill().await;
            RunOutcome {
                passed: false,
                error: Some(format!("timed out after {}s", timeout.as_secs())),
                duration,
                timed_out: true,
                scenarios: collected.into_scenarios(false),
            }
        }
    }
}

/// Append the child's last stderr line (the actual reason) to a failure message,
/// if there is one.
fn with_reason(msg: &str, stderr_tail: &str) -> String {
    match stderr_tail.lines().rfind(|l| !l.trim().is_empty()) {
        Some(reason) => format!("{msg}: {}", reason.trim()),
        None => msg.to_string(),
    }
}

/// Read the child's NDJSON stdout line by line into a [`Collected`].
async fn collect(stdout: tokio::process::ChildStdout) -> Collected {
    let mut collected = Collected::default();
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        collected.fold_line(&line);
    }
    collected
}

/// Collect the child's stderr, keeping only the last few lines — the tail is
/// where the failure reason lands, and bounding it stops a noisy child from
/// growing this unbounded.
async fn collect_lines(stderr: tokio::process::ChildStderr) -> String {
    use std::collections::VecDeque;
    const MAX: usize = 20;
    let mut tail: VecDeque<String> = VecDeque::with_capacity(MAX);
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if tail.len() == MAX {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    tail.into_iter().collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg() -> MonitorConfig {
        MonitorConfig {
            name: "smoke".into(),
            path: PathBuf::from("scenarios/smoke.rhai"),
            schedule: None,
            timeout: None,
            env_file: vec![PathBuf::from("ci.env")],
            scenario: Some("answered".into()),
            tags: vec!["smoke".into()],
        }
    }

    #[test]
    fn build_args_wires_flags_and_filters() {
        let args = build_args(&cfg());
        assert_eq!(&args[0..4], &["run", "--json", "--metrics", "--no-color"]);
        assert!(args.contains(&"scenarios/smoke.rhai".to_string()));
        assert!(args.windows(2).any(|w| w == ["--env-file", "ci.env"]));
        assert!(args.windows(2).any(|w| w == ["--scenario", "answered"]));
        assert!(args.windows(2).any(|w| w == ["--tag", "smoke"]));
    }

    #[test]
    fn groups_agents_by_scenario_in_order() {
        let mut c = Collected::default();
        // A suite: two scenarios, each with two agents, then per-scenario results.
        c.fold_line(r#"{"event":"metric","scenario":"rejects","agent":"Caller","registered":true,"mos":4.4}"#);
        c.fold_line(
            r#"{"event":"metric","scenario":"rejects","agent":"Callee","registered":true}"#,
        );
        c.fold_line(r#"{"event":"scenario_finished","name":"rejects","passed":true}"#);
        c.fold_line(r#"{"event":"metric","scenario":"accepts","agent":"Caller","registered":true,"mos":4.3,"jitter_ms":8.0}"#);
        c.fold_line(r#"{"event":"scenario_finished","name":"accepts","passed":false}"#);
        // Noise is ignored.
        c.fold_line("not json");
        c.fold_line(r#"{"event":"wait","seconds":1.0}"#);

        let scenarios = c.into_scenarios(true);
        assert_eq!(scenarios.len(), 2);
        // First-seen order preserved.
        assert_eq!(scenarios[0].name, "rejects");
        assert_eq!(scenarios[0].agents.len(), 2);
        assert!(scenarios[0].passed);
        assert_eq!(scenarios[1].name, "accepts");
        assert_eq!(scenarios[1].agents.len(), 1);
        assert!(!scenarios[1].passed); // per-scenario fail despite overall=true
        assert_eq!(scenarios[1].agents[0].jitter_ms, Some(8.0));
    }

    #[test]
    fn single_scenario_falls_back_to_overall_pass() {
        let mut c = Collected::default();
        // A single-scenario file emits `metric` (scenario = its name) but no
        // `scenario_finished` — pass/fall comes from the overall exit code.
        c.fold_line(r#"{"event":"metric","scenario":"the call","agent":"A","registered":true}"#);
        let scenarios = c.into_scenarios(false);
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].name, "the call");
        assert!(!scenarios[0].passed);
    }
}
