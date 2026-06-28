//! The format-neutral metrics store: aggregates runs into a `monitor` → `scenario`
//! → `agent` hierarchy of counters and last-run gauges. A format module (e.g.
//! [`super::prometheus`]) reads this and renders it; adding a new output format
//! means a new module over the same store, not touching this one.
//!
//! Each run replaces the monitor's scenario/agent snapshot, so stale series
//! (a scenario/agent a later run no longer produced) simply stop being read.

use super::runner::RunOutcome;
use std::collections::BTreeMap;

/// All recorded metrics, keyed by monitor name. `BTreeMap` keeps a renderer's
/// output stable (sorted) across reads.
#[derive(Default)]
pub(super) struct MetricsStore {
    pub(super) monitors: BTreeMap<String, MonitorMetrics>,
}

/// Aggregated metrics for one monitor.
#[derive(Default)]
pub(super) struct MonitorMetrics {
    pub(super) runs_pass: u64,
    pub(super) runs_fail: u64,
    pub(super) runs_timeout: u64,
    pub(super) last_success: bool,
    pub(super) last_duration_s: f64,
    pub(super) last_run_unix: i64,
    /// The last run's scenarios, keyed by scenario name (sorted for stable output).
    pub(super) scenarios: BTreeMap<String, ScenarioMetrics>,
}

/// The last run's metrics for one scenario.
#[derive(Default)]
pub(super) struct ScenarioMetrics {
    pub(super) passed: bool,
    /// Per-agent quality, keyed by agent name.
    pub(super) agents: BTreeMap<String, AgentSample>,
}

/// The last quality sample for one agent.
pub(super) struct AgentSample {
    pub(super) registered: bool,
    pub(super) mos: Option<f64>,
    pub(super) jitter_ms: Option<f64>,
    pub(super) packet_loss_pct: Option<f64>,
    pub(super) rtt_ms: Option<f64>,
}

impl MetricsStore {
    /// Fold a finished run into the store. `now_unix` is the run's completion
    /// time (passed in so this stays free of wall-clock calls).
    pub(super) fn record(&mut self, monitor: &str, outcome: &RunOutcome, now_unix: i64) {
        let m = self.monitors.entry(monitor.to_string()).or_default();
        if outcome.passed {
            m.runs_pass += 1;
        } else {
            m.runs_fail += 1;
        }
        if outcome.timed_out {
            m.runs_timeout += 1;
        }
        m.last_success = outcome.passed;
        m.last_duration_s = outcome.duration.as_secs_f64();
        m.last_run_unix = now_unix;
        // Replace the scenario/agent snapshot with this run's.
        m.scenarios = outcome
            .scenarios
            .iter()
            .map(|s| {
                let agents = s
                    .agents
                    .iter()
                    .map(|a| {
                        (
                            a.agent.clone(),
                            AgentSample {
                                registered: a.registered,
                                mos: a.mos,
                                jitter_ms: a.jitter_ms,
                                packet_loss_pct: a.packet_loss_pct,
                                rtt_ms: a.rtt_ms,
                            },
                        )
                    })
                    .collect();
                (
                    s.name.clone(),
                    ScenarioMetrics {
                        passed: s.passed,
                        agents,
                    },
                )
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serve::runner::{AgentMetric, ScenarioOutcome};
    use std::time::Duration;

    #[test]
    fn record_counts_and_snapshots_last_run() {
        let mut store = MetricsStore::default();
        let outcome = RunOutcome {
            passed: true,
            error: None,
            duration: Duration::from_millis(1500),
            timed_out: false,
            scenarios: vec![ScenarioOutcome {
                name: "rejects".into(),
                passed: true,
                agents: vec![AgentMetric {
                    agent: "Caller".into(),
                    registered: true,
                    mos: Some(4.2),
                    jitter_ms: None,
                    packet_loss_pct: None,
                    rtt_ms: None,
                }],
            }],
        };
        store.record("smoke", &outcome, 1_700_000_000);

        let m = &store.monitors["smoke"];
        assert_eq!(m.runs_pass, 1);
        assert_eq!(m.runs_fail, 0);
        assert!(m.last_success);
        let s = &m.scenarios["rejects"];
        assert!(s.passed);
        assert_eq!(s.agents["Caller"].mos, Some(4.2));
    }

    #[test]
    fn record_replaces_scenarios_each_run() {
        let mut store = MetricsStore::default();
        let mk = |scenario: &str| RunOutcome {
            passed: false,
            error: None,
            duration: Duration::from_millis(1),
            timed_out: false,
            scenarios: vec![ScenarioOutcome {
                name: scenario.into(),
                passed: false,
                agents: vec![],
            }],
        };
        store.record("m", &mk("first"), 1);
        store.record("m", &mk("second"), 2);
        let m = &store.monitors["m"];
        // Counter accumulates, but the scenario snapshot is the latest run's only.
        assert_eq!(m.runs_fail, 2);
        assert!(!m.scenarios.contains_key("first"));
        assert!(m.scenarios.contains_key("second"));
    }
}
