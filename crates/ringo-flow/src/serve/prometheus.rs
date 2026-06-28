//! Renders the [`MetricsStore`] in the Prometheus text exposition format. The
//! store is format-neutral, so a second output format would be a sibling module
//! with its own `render` — this module is the only place that knows Prometheus.

use super::metrics::{AgentSample, MetricsStore, MonitorMetrics};
use std::collections::BTreeMap;

/// The content type for the Prometheus text exposition format.
pub(super) const CONTENT_TYPE: &str = "text/plain; version=0.0.4";

/// Render the whole store in the Prometheus text exposition format.
pub(super) fn render(store: &MetricsStore) -> String {
    let monitors = &store.monitors;
    let mut out = String::new();

    help(
        &mut out,
        "ringo_monitor_runs_total",
        "counter",
        "Total monitor runs by result.",
    );
    for (name, m) in monitors {
        line(
            &mut out,
            "ringo_monitor_runs_total",
            &[("monitor", name), ("result", "pass")],
            m.runs_pass as f64,
        );
        line(
            &mut out,
            "ringo_monitor_runs_total",
            &[("monitor", name), ("result", "fail")],
            m.runs_fail as f64,
        );
        line(
            &mut out,
            "ringo_monitor_runs_total",
            &[("monitor", name), ("result", "timeout")],
            m.runs_timeout as f64,
        );
    }

    help(
        &mut out,
        "ringo_monitor_last_success",
        "gauge",
        "Whether the most recent run passed (1) or failed (0).",
    );
    for (name, m) in monitors {
        line(
            &mut out,
            "ringo_monitor_last_success",
            &[("monitor", name)],
            b2f(m.last_success),
        );
    }

    help(
        &mut out,
        "ringo_monitor_last_duration_seconds",
        "gauge",
        "Wall-clock duration of the most recent run.",
    );
    for (name, m) in monitors {
        line(
            &mut out,
            "ringo_monitor_last_duration_seconds",
            &[("monitor", name)],
            m.last_duration_s,
        );
    }

    help(
        &mut out,
        "ringo_monitor_last_run_timestamp_seconds",
        "gauge",
        "Unix time of the most recent run's completion.",
    );
    for (name, m) in monitors {
        line(
            &mut out,
            "ringo_monitor_last_run_timestamp_seconds",
            &[("monitor", name)],
            m.last_run_unix as f64,
        );
    }

    help(
        &mut out,
        "ringo_scenario_last_success",
        "gauge",
        "Whether the scenario passed (1) or failed (0) in the most recent run.",
    );
    for (monitor, m) in monitors {
        for (scenario, s) in &m.scenarios {
            line(
                &mut out,
                "ringo_scenario_last_success",
                &[("monitor", monitor), ("scenario", scenario)],
                b2f(s.passed),
            );
        }
    }

    // Per-agent gauges. A field absent for a run (no measurable call) is simply
    // not emitted that scrape.
    help(
        &mut out,
        "ringo_agent_registered",
        "gauge",
        "Whether the agent was registered at the run's end (1/0).",
    );
    for_each_agent(monitors, &mut out, "ringo_agent_registered", |a| {
        Some(b2f(a.registered))
    });

    help(
        &mut out,
        "ringo_call_mos",
        "gauge",
        "Estimated Mean Opinion Score (1.0–4.5).",
    );
    for_each_agent(monitors, &mut out, "ringo_call_mos", |a| a.mos);

    help(
        &mut out,
        "ringo_call_jitter_ms",
        "gauge",
        "Receive-side jitter, milliseconds.",
    );
    for_each_agent(monitors, &mut out, "ringo_call_jitter_ms", |a| a.jitter_ms);

    help(
        &mut out,
        "ringo_call_packet_loss_pct",
        "gauge",
        "Receive-side packet loss, percent.",
    );
    for_each_agent(monitors, &mut out, "ringo_call_packet_loss_pct", |a| {
        a.packet_loss_pct
    });

    help(
        &mut out,
        "ringo_call_rtt_ms",
        "gauge",
        "Round-trip time, milliseconds.",
    );
    for_each_agent(monitors, &mut out, "ringo_call_rtt_ms", |a| a.rtt_ms);

    out
}

/// Emit one gauge line per agent for the values returned by `pick` (skipping
/// agents where `pick` returns `None`), labelled `monitor`/`scenario`/`agent`.
fn for_each_agent(
    monitors: &BTreeMap<String, MonitorMetrics>,
    out: &mut String,
    metric: &str,
    pick: impl Fn(&AgentSample) -> Option<f64>,
) {
    for (monitor, m) in monitors {
        for (scenario, s) in &m.scenarios {
            for (agent, sample) in &s.agents {
                if let Some(v) = pick(sample) {
                    line(
                        out,
                        metric,
                        &[
                            ("monitor", monitor),
                            ("scenario", scenario),
                            ("agent", agent),
                        ],
                        v,
                    );
                }
            }
        }
    }
}

/// Append a `# HELP`/`# TYPE` header for a metric.
fn help(out: &mut String, metric: &str, kind: &str, help: &str) {
    out.push_str(&format!("# HELP {metric} {help}\n# TYPE {metric} {kind}\n"));
}

/// Append one sample line: `metric{label="v",…} value`.
fn line(out: &mut String, metric: &str, labels: &[(&str, &str)], value: f64) {
    out.push_str(metric);
    if !labels.is_empty() {
        out.push('{');
        for (i, (k, v)) in labels.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!("{k}=\"{}\"", escape(v)));
        }
        out.push('}');
    }
    out.push_str(&format!(" {value}\n"));
}

fn b2f(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

/// Escape a Prometheus label value (`\`, `"`, newline) per the exposition format.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serve::runner::{AgentMetric, RunOutcome, ScenarioOutcome};
    use std::time::Duration;

    fn store_with(passed: bool, scenarios: Vec<ScenarioOutcome>) -> MetricsStore {
        let mut store = MetricsStore::default();
        store.record(
            "smoke",
            &RunOutcome {
                passed,
                error: None,
                duration: Duration::from_millis(1500),
                timed_out: false,
                scenarios,
            },
            1_700_000_000,
        );
        store
    }

    #[test]
    fn renders_nested_labels() {
        let store = store_with(
            true,
            vec![ScenarioOutcome {
                name: "callee rejects".into(),
                passed: true,
                agents: vec![AgentMetric {
                    agent: "Caller".into(),
                    registered: true,
                    mos: Some(4.2),
                    jitter_ms: Some(2.0),
                    packet_loss_pct: Some(0.0),
                    rtt_ms: Some(18.0),
                }],
            }],
        );
        let out = render(&store);
        assert!(
            out.contains(r#"ringo_monitor_runs_total{monitor="smoke",result="pass"} 1"#),
            "{out}"
        );
        assert!(
            out.contains(
                r#"ringo_scenario_last_success{monitor="smoke",scenario="callee rejects"} 1"#
            ),
            "{out}"
        );
        assert!(
            out.contains(
                r#"ringo_call_mos{monitor="smoke",scenario="callee rejects",agent="Caller"} 4.2"#
            ),
            "{out}"
        );
    }

    #[test]
    fn absent_quality_field_is_skipped() {
        let store = store_with(
            false,
            vec![ScenarioOutcome {
                name: "registers".into(),
                passed: false,
                agents: vec![AgentMetric {
                    agent: "a".into(),
                    registered: false,
                    mos: None,
                    jitter_ms: None,
                    packet_loss_pct: None,
                    rtt_ms: None,
                }],
            }],
        );
        let out = render(&store);
        assert!(
            out.contains(
                r#"ringo_agent_registered{monitor="smoke",scenario="registers",agent="a"} 0"#
            ),
            "{out}"
        );
        // No call → no MOS line for this agent.
        assert!(!out.contains("ringo_call_mos{monitor=\"smoke\""), "{out}");
    }
}
