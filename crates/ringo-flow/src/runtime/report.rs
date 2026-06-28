//! Output is decoupled from the runner: the runner emits semantic [`Event`]s and
//! a [`Reporter`] decides how to render them. [`Human`] prints a readable log;
//! [`Json`] emits one JSON object per line (NDJSON) for machines/CI.

use serde::Serialize;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether ANSI styling (bold/color) may be emitted at all. Combined with a
/// per-stream TTY check; turned off by `--no-color`/`NO_COLOR`.
static ANSI_ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable/disable ANSI styling globally (set once at startup from the CLI flag
/// and the `NO_COLOR` environment variable).
pub fn set_ansi_enabled(on: bool) {
    ANSI_ENABLED.store(on, Ordering::Relaxed);
}

/// A semantic thing that happened while running a scenario. The single source
/// for both the human log and the JSON stream.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event<'a> {
    AgentStarted {
        name: &'a str,
        aor: &'a str,
    },
    /// A command was issued: `kind` is register/dial/accept/hangup/hold/resume/
    /// mute/dtmf; `detail` carries the target/digits where relevant.
    Action {
        agent: &'a str,
        kind: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<&'a str>,
    },
    Wait {
        seconds: f64,
    },
    /// A free-form note emitted by the scenario via `log(...)`.
    Log {
        message: &'a str,
    },
    /// An HTTP request was made (and got a response status).
    Http {
        method: &'a str,
        url: &'a str,
        status: u16,
    },
    /// A mock server received an HTTP request (and whether a route matched it).
    MockRequest {
        method: &'a str,
        path: &'a str,
        matched: bool,
    },
    /// A mock responder failed (logged here rather than exposed over HTTP).
    MockError {
        method: &'a str,
        path: &'a str,
        error: &'a str,
    },
    /// Per-agent media-quality metrics, emitted at a scenario's end when
    /// `--metrics` is set. Machine consumers (`serve`) feed these into
    /// Prometheus; the quality fields are absent when the agent had no
    /// measurable call.
    Metric {
        scenario: &'a str,
        agent: &'a str,
        registered: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        mos: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        jitter_ms: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        packet_loss_pct: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        rtt_ms: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        rx_lost: Option<i64>,
    },
    Assertion {
        /// Optional `.describe(...)` label, prefixed to the log line; `None` if unset.
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<&'a str>,
        expect: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        actual: Option<String>,
    },
    /// A scenario file began (only when running more than one file).
    FileStarted {
        path: &'a str,
    },
    /// A scenario (test-suite case) began.
    ScenarioStarted {
        name: &'a str,
    },
    /// A scenario finished with a pass/fail (and reason on failure).
    ScenarioFinished {
        name: &'a str,
        passed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// A scenario was skipped (statically via `#{ skip }` or at runtime via `skip`).
    ScenarioSkipped {
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<&'a str>,
    },
    /// A suite of scenarios finished: how many passed/skipped of the total run.
    SuiteFinished {
        total: usize,
        passed: usize,
        skipped: usize,
    },
    Finished {
        passed: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// A whole multi-file run finished: totals across all files.
    RunFinished {
        files: usize,
        passed_files: usize,
        scenarios: usize,
        passed_scenarios: usize,
        skipped_scenarios: usize,
    },
}

pub trait Reporter {
    fn emit(&mut self, event: &Event);
}

/// How much the [`Human`] reporter prints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Level {
    /// File/scenario headers, failures and the final result (no per-step noise).
    Quiet,
    /// Steps, assertions and the result (default).
    Normal,
    /// Adds the observed state to every assertion.
    Verbose,
}

/// Human-readable progress log (the default), filtered by [`Level`].
pub struct Human {
    level: Level,
}

impl Human {
    pub fn new(level: Level) -> Self {
        Self { level }
    }
}

impl Reporter for Human {
    fn emit(&mut self, event: &Event) {
        let normal = self.level != Level::Quiet;
        match event {
            Event::AgentStarted { name, aor } if normal => {
                out(Some(name), &format!("starting ({aor})"))
            }
            Event::Action {
                agent,
                kind,
                detail,
            } if normal => out(Some(agent), &action_line(kind, *detail)),
            Event::Wait { seconds } if normal => out(None, &format!("wait {seconds}s")),
            // A user note prints at every level except quiet.
            Event::Log { message } if normal => out(None, message),
            Event::Http {
                method,
                url,
                status,
            } if normal => out(None, &format!("HTTP {method} {url} → {status}")),
            Event::MockRequest {
                method,
                path,
                matched,
            } if normal => out(
                None,
                &format!(
                    "mock ← {method} {path}{}",
                    if *matched { "" } else { " (no route → 404)" }
                ),
            ),
            // Responder failures print (to stderr) at every level — they're scenario
            // bugs the author needs to see, and the HTTP caller only got a bare 500.
            Event::MockError {
                method,
                path,
                error,
            } => err(
                None,
                &format!("{} mock {method} {path} responder: {error}", fail_mark()),
            ),
            Event::Metric {
                agent,
                registered,
                mos,
                jitter_ms,
                packet_loss_pct,
                ..
            } if normal => {
                let quality = match mos {
                    Some(m) => format!(
                        "MOS {m:.2}, jitter {:.1}ms, loss {:.1}%",
                        jitter_ms.unwrap_or(0.0),
                        packet_loss_pct.unwrap_or(0.0)
                    ),
                    None => "no call".to_string(),
                };
                out(
                    Some(agent),
                    &format!("metrics: registered={registered}, {quality}"),
                )
            }
            Event::Assertion {
                label,
                expect,
                ok,
                actual,
            } => {
                let actual = actual.as_deref().unwrap_or("?");
                if !ok {
                    // Failures print (to stderr) at every level.
                    err(
                        *label,
                        &format!("{} expect {expect} — actual: {actual}", fail_mark()),
                    );
                } else if self.level == Level::Verbose {
                    out(
                        *label,
                        &format!("{} expect {expect} — actual: {actual}", ok_mark()),
                    );
                } else if normal {
                    out(*label, &format!("{} expect {expect}", ok_mark()));
                }
            }
            // File/scenario headers print at every level (incl. quiet) so you can
            // always see what's running — quiet only drops the per-step noise.
            Event::FileStarted { path } => {
                // Heavier marker than a scenario, to group a file's scenarios.
                println!();
                out(None, &emphasize(&format!("▶▶ {path}")));
            }
            Event::ScenarioStarted { name } => {
                println!(); // blank line sets each scenario apart in the stream
                out(None, &emphasize(&format!("▶ {name}")));
            }
            Event::ScenarioFinished {
                name,
                passed,
                error,
            } => {
                if *passed {
                    if normal {
                        out(
                            None,
                            &styled(out_tty(), "32", &format!("✓ scenario `{name}`")),
                        );
                    }
                } else {
                    let detail = error
                        .as_deref()
                        .map(|e| format!(" — {e}"))
                        .unwrap_or_default();
                    err(
                        None,
                        &styled(err_tty(), "31", &format!("✗ scenario `{name}`{detail}")),
                    );
                }
            }
            Event::ScenarioSkipped { name, reason } => {
                let detail = reason.map(|r| format!(" ({r})")).unwrap_or_default();
                out(
                    None,
                    &styled(
                        out_tty(),
                        "33",
                        &format!("⊘ scenario `{name}` skipped{detail}"),
                    ),
                );
            }
            Event::SuiteFinished {
                total,
                passed,
                skipped,
            } => {
                let failed = total.saturating_sub(*passed).saturating_sub(*skipped);
                let body = tally(*total, *passed, failed, *skipped);
                // Blank line + bold so the final tally stands out at the end.
                if failed == 0 {
                    println!();
                    out(None, &styled(out_tty(), "1;32", &format!("✓ {body}")));
                } else {
                    eprintln!();
                    err(None, &styled(err_tty(), "1;31", &format!("✗ {body}")));
                }
            }
            Event::Finished { passed, error } => {
                if *passed {
                    println!();
                    out(None, &styled(out_tty(), "1;32", "✓ scenario passed"));
                } else {
                    let detail = error
                        .as_deref()
                        .map(|e| format!(": {e}"))
                        .unwrap_or_default();
                    eprintln!();
                    err(
                        None,
                        &styled(err_tty(), "1;31", &format!("✗ scenario failed{detail}")),
                    );
                }
            }
            Event::RunFinished {
                files,
                passed_files,
                scenarios,
                passed_scenarios,
                skipped_scenarios,
            } => {
                let skip_note = if *skipped_scenarios > 0 {
                    format!(" ({skipped_scenarios} skipped)")
                } else {
                    String::new()
                };
                let body = format!(
                    "{files} files, {scenarios} scenarios — {passed_scenarios}/{scenarios} scenarios{skip_note}, {passed_files}/{files} files passed"
                );
                if passed_files == files {
                    println!();
                    out(None, &styled(out_tty(), "1;32", &format!("✓ {body}")));
                } else {
                    eprintln!();
                    err(None, &styled(err_tty(), "1;31", &format!("✗ {body}")));
                }
            }
            // Suppressed at this level.
            _ => {}
        }
    }
}

/// Short wall-clock timestamp for the human log (correlates with baresip's `-s`
/// trace, which uses the same `HH:MM:SS.mmm` format).
fn human_ts() -> String {
    chrono::Local::now().format("%H:%M:%S%.3f").to_string()
}

/// Wrap `s` in SGR `codes` (e.g. `"1"` bold, `"32"` green, `"1;31"` bold-red) when
/// styling is enabled and `tty`; otherwise return it plain (pipes/files/`--no-color`).
fn styled(tty: bool, codes: &str, s: &str) -> String {
    if ANSI_ENABLED.load(Ordering::Relaxed) && tty {
        format!("\x1b[{codes}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn out_tty() -> bool {
    std::io::stdout().is_terminal()
}
fn err_tty() -> bool {
    std::io::stderr().is_terminal()
}

/// Bold for stdout headers / pass summaries.
fn emphasize(s: &str) -> String {
    styled(out_tty(), "1", s)
}

/// A green `✓` (stdout) / red `✗` (stderr) marker for pass/fail lines.
fn ok_mark() -> String {
    styled(out_tty(), "32", "✓")
}
fn fail_mark() -> String {
    styled(err_tty(), "31", "✗")
}

/// Print one human line: `<ts> [<agent>: ]<body>` to stdout.
fn out(agent: Option<&str>, body: &str) {
    match agent {
        Some(a) => println!("{} {a}: {body}", human_ts()),
        None => println!("{} {body}", human_ts()),
    }
}

/// Same as [`out`] but to stderr (used for failures).
fn err(agent: Option<&str>, body: &str) {
    match agent {
        Some(a) => eprintln!("{} {a}: {body}", human_ts()),
        None => eprintln!("{} {body}", human_ts()),
    }
}

/// The suite tally line: `N scenarios — P passed, F failed[, S skipped]`.
fn tally(total: usize, passed: usize, failed: usize, skipped: usize) -> String {
    let mut s = format!("{total} scenarios — {passed} passed, {failed} failed");
    if skipped > 0 {
        s.push_str(&format!(", {skipped} skipped"));
    }
    s
}

/// The body of an agent action line (the agent name is the line prefix).
fn action_line(kind: &str, detail: Option<&str>) -> String {
    match kind {
        "register" => "register".to_string(),
        "dial" => format!("dials {}", detail.unwrap_or_default()),
        "accept" => "accepts".to_string(),
        "hangup" => "hangs up".to_string(),
        "hold" => "holds".to_string(),
        "resume" => "resumes".to_string(),
        "mute" => "toggles mute".to_string(),
        "dtmf-start" => format!("sending DTMF {}", detail.unwrap_or_default()),
        "dtmf-done" => format!("sent DTMF {}", detail.unwrap_or_default()),
        "header" => format!("set header {}", detail.unwrap_or_default()),
        "send-audio" => format!("sends {}", detail.unwrap_or_default()),
        other => other.to_string(),
    }
}

/// NDJSON: one JSON object per event on stdout.
pub struct Json;

impl Reporter for Json {
    fn emit(&mut self, event: &Event) {
        match serde_json::to_value(event) {
            Ok(mut value) => {
                if let Some(obj) = value.as_object_mut() {
                    // RFC3339 timestamp for machine consumers / CI correlation.
                    obj.insert("ts".into(), chrono::Local::now().to_rfc3339().into());
                }
                println!("{value}");
            }
            Err(e) => eprintln!("(failed to serialize event: {e})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assertion_serializes_tagged() {
        let json = serde_json::to_string(&Event::Assertion {
            label: Some("caller registered"),
            expect: "state is ringing".into(),
            ok: false,
            actual: Some("idle".into()),
        })
        .unwrap();
        assert!(json.contains(r#""event":"assertion""#), "{json}");
        assert!(json.contains(r#""ok":false"#), "{json}");
        assert!(json.contains(r#""actual":"idle""#), "{json}");
        assert!(json.contains(r#""label":"caller registered""#), "{json}");
    }

    #[test]
    fn metric_omits_absent_quality_fields() {
        // No call → only scenario/agent/registered, quality fields skipped.
        let json = serde_json::to_string(&Event::Metric {
            scenario: "smoke",
            agent: "caller",
            registered: true,
            mos: None,
            jitter_ms: None,
            packet_loss_pct: None,
            rtt_ms: None,
            rx_lost: None,
        })
        .unwrap();
        assert!(json.contains(r#""event":"metric""#), "{json}");
        assert!(json.contains(r#""registered":true"#), "{json}");
        assert!(!json.contains("mos"), "{json}");
        assert!(!json.contains("jitter_ms"), "{json}");
    }

    #[test]
    fn metric_serializes_quality_when_present() {
        let json = serde_json::to_string(&Event::Metric {
            scenario: "call quality",
            agent: "callee",
            registered: true,
            mos: Some(4.3),
            jitter_ms: Some(2.5),
            packet_loss_pct: Some(0.0),
            rtt_ms: Some(18.0),
            rx_lost: Some(0),
        })
        .unwrap();
        assert!(json.contains(r#""mos":4.3"#), "{json}");
        assert!(json.contains(r#""rx_lost":0"#), "{json}");
    }

    #[test]
    fn action_without_detail_omits_field() {
        let json = serde_json::to_string(&Event::Action {
            agent: "A",
            kind: "register",
            detail: None,
        })
        .unwrap();
        assert!(!json.contains("detail"), "{json}");
    }
}
