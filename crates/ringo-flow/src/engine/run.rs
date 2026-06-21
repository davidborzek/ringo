//! The language-neutral runner: a [`ScriptHost`] plugs a scripting language into
//! the engine. `run` owns the runtime, the per-scenario isolation, teardown and
//! reporting; the host only knows how to evaluate the top-level pass and a single
//! scenario. Adding a language means implementing [`ScriptHost`] — nothing here
//! changes.

use super::ctx::Ctx;
use crate::runtime::Output;
use crate::runtime::report::{Event, Human, Json, Level, Reporter};
use crate::runtime::session::AgentSession;
use anyhow::{Result, anyhow};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// What a scripting language must provide to be run by [`run`]. The host owns its
/// engine/compiled program and the `Arc<Ctx>` it shares with the script verbs.
pub trait ScriptHost {
    /// Evaluate the top-level pass (which registers any scenarios). Returns
    /// whether the file is a single top-level scenario or a suite of named ones.
    fn run_top_level(&mut self) -> TopLevel;
    /// Run one named scenario in isolation (`setup` → body → `teardown`).
    fn run_scenario(&mut self, name: &str) -> std::result::Result<(), String>;
}

/// Result of the top-level pass.
pub enum TopLevel {
    /// No scenarios registered → the top-level code itself was the scenario.
    Single(std::result::Result<(), String>),
    /// Scenarios were registered; `top_error` carries a top-level pass failure.
    Suite {
        names: Vec<String>,
        top_error: Option<String>,
    },
}

/// A single top-level scenario, or a suite of named scenarios.
enum Outcome {
    Single(std::result::Result<(), String>),
    Suite { total: usize, passed: usize },
}

/// Run one or more scenario programs. Builds the runtime + reporter + [`Ctx`]
/// once; each program (a file) is compiled by its `build` closure (no eval yet,
/// so syntax errors surface here), then driven on a blocking thread (verbs
/// `block_on`). One program behaves exactly like before; several are run in
/// sequence (sessions reset between them) with per-file results and an overall
/// summary. `programs` is `(label, build)` pairs.
pub fn run<H, F>(
    programs: Vec<(String, F)>,
    output: Output,
    default_timeout: Duration,
    only: Option<String>,
) -> Result<()>
where
    H: ScriptHost + Send + 'static,
    F: FnOnce(Arc<Ctx>) -> Result<H> + Send + 'static,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let level = if output.quiet {
        Level::Quiet
    } else if output.verbose {
        Level::Verbose
    } else {
        Level::Normal
    };
    let reporter: Box<dyn Reporter + Send> = if output.json {
        Box::new(Json)
    } else {
        Box::new(Human::new(level))
    };

    // `--scenario` is a case-insensitive substring by default (no escaping of
    // names like "transfer (blind refer)"); `re:` opts into a full regex. Built
    // here so a bad regex fails before anything starts.
    let matcher = only.as_deref().map(build_matcher).transpose()?;

    let ctx = Arc::new(Ctx::new(rt.handle().clone(), reporter, default_timeout));
    if output.insecure_http {
        ctx.set_http_insecure(true);
        eprintln!(
            "⚠ ringo-flow: TLS certificate verification is DISABLED for http(...) (--insecure-http)"
        );
    }
    let (logs, save_audio) = (output.logs, output.save_audio);
    let multi = programs.len() > 1;

    // NOT `?` on the join — cleanup must run even if a script panics, so baresip
    // gets clean BYEs and a terminal event is always emitted. Everything runs on
    // the blocking thread because verbs `block_on`.
    let c = ctx.clone();
    let join = rt.block_on(async move {
        tokio::task::spawn_blocking(move || {
            run_files(programs, &c, matcher.as_ref(), logs, save_audio, multi)
        })
        .await
    });

    let agg = match join {
        Ok(agg) => agg,
        Err(e) => {
            // The script thread panicked: still clean up (clean BYEs) and emit a
            // terminal failure event before surfacing the error.
            teardown(&ctx, &rt);
            let msg = format!("script task panicked: {e}");
            ctx.emit(&Event::Finished {
                passed: false,
                error: Some(msg.clone()),
            });
            return Err(anyhow!(msg));
        }
    };

    teardown(&ctx, &rt);

    // A filter that matched nothing is almost always a mistake — fail loudly
    // rather than silently "pass" 0 scenarios.
    if let Some(pat) = &only
        && agg.scenarios == 0
    {
        return Err(anyhow!("no scenario matched --scenario pattern `{pat}`"));
    }

    if multi {
        ctx.emit(&Event::RunFinished {
            files: agg.files,
            passed_files: agg.passed_files,
            scenarios: agg.scenarios,
            passed_scenarios: agg.passed_scenarios,
        });
    }

    if agg.passed_files == agg.files {
        Ok(())
    } else {
        Err(anyhow!(
            "{}/{} files failed",
            agg.files - agg.passed_files,
            agg.files
        ))
    }
}

/// Totals across the files of a run.
#[derive(Default)]
struct Aggregate {
    files: usize,
    passed_files: usize,
    scenarios: usize,
    passed_scenarios: usize,
}

/// Run each program (file) in sequence on the current (blocking) thread: build
/// it, drive its lifecycle, dump artifacts, then reset sessions before the next.
/// Emits each file's per-scenario + terminal events (and a `FileStarted` header
/// when running more than one), and returns the totals.
fn run_files<H, F>(
    programs: Vec<(String, F)>,
    ctx: &Arc<Ctx>,
    matcher: Option<&ScenarioMatcher>,
    logs: bool,
    save_audio: bool,
    multi: bool,
) -> Aggregate
where
    H: ScriptHost,
    F: FnOnce(Arc<Ctx>) -> Result<H>,
{
    let mut agg = Aggregate::default();
    for (label, build) in programs {
        agg.files += 1;
        if multi {
            ctx.emit(&Event::FileStarted { path: &label });
        }
        let host = match build(ctx.clone()) {
            Ok(h) => h,
            Err(e) => {
                // A compile/build error fails just this file; keep going.
                ctx.emit(&Event::Finished {
                    passed: false,
                    error: Some(format!("{e}")),
                });
                continue;
            }
        };
        let outcome = run_lifecycle(host, ctx, matcher, logs, save_audio);
        ctx.reset_sessions(); // isolate the next file (fresh agents)

        let (total, passed) = match outcome {
            Outcome::Single(r) => {
                let ok = r.is_ok();
                ctx.emit(&Event::Finished {
                    passed: ok,
                    error: r.err(),
                });
                (1, usize::from(ok))
            }
            Outcome::Suite { total, passed } => {
                ctx.emit(&Event::SuiteFinished { total, passed });
                (total, passed)
            }
        };
        agg.scenarios += total;
        agg.passed_scenarios += passed;
        if passed == total {
            agg.passed_files += 1;
        }
    }
    agg
}

/// A compiled `--scenario` filter (substring or regex).
type ScenarioMatcher = Box<dyn Fn(&str) -> bool + Send>;

/// Build a scenario-name matcher from a `--scenario` pattern: a case-insensitive
/// substring by default, or a full regex if prefixed with `re:` (errors on a bad
/// regex). Substring avoids escaping metacharacters in names like
/// `transfer (blind refer)`.
fn build_matcher(pattern: &str) -> Result<ScenarioMatcher> {
    if let Some(src) = pattern.strip_prefix("re:") {
        let re = Regex::new(src).map_err(|e| anyhow!("invalid --scenario regex `{src}`: {e}"))?;
        Ok(Box::new(move |name| re.is_match(name)))
    } else {
        let needle = pattern.to_lowercase();
        Ok(Box::new(move |name| name.to_lowercase().contains(&needle)))
    }
}

/// Top-level pass, then the single top-level scenario or each registered one in
/// isolation (fresh agents between scenarios via [`Ctx::reset_sessions`]).
/// `--logs`/`--save-audio` are dumped while the sessions still exist (before the
/// per-scenario reset / final teardown).
fn run_lifecycle<H: ScriptHost>(
    mut host: H,
    ctx: &Arc<Ctx>,
    only: Option<&ScenarioMatcher>,
    logs: bool,
    save_audio: bool,
) -> Outcome {
    let names = match host.run_top_level() {
        TopLevel::Single(r) => {
            dump_artifacts(ctx, logs, save_audio); // single mode: sessions persist to teardown
            return Outcome::Single(r);
        }
        TopLevel::Suite {
            top_error: Some(e), ..
        } => return Outcome::Single(Err(e)),
        TopLevel::Suite { names, .. } => names,
    };

    let (mut total, mut passed) = (0, 0);
    for name in names {
        if only.is_some_and(|m| !m(&name)) {
            continue;
        }
        total += 1;
        ctx.emit(&Event::ScenarioStarted { name: &name });
        let result = host.run_scenario(&name);
        dump_artifacts(ctx, logs, save_audio); // before reset drops this scenario's sessions
        ctx.reset_sessions(); // isolation: fresh agents for the next scenario
        match &result {
            Ok(()) => {
                passed += 1;
                ctx.emit(&Event::ScenarioFinished {
                    name: &name,
                    passed: true,
                    error: None,
                });
            }
            Err(msg) => ctx.emit(&Event::ScenarioFinished {
                name: &name,
                passed: false,
                error: Some(msg.clone()),
            }),
        }
    }
    Outcome::Suite { total, passed }
}

/// Dump `--logs` / `--save-audio` for the currently-live sessions. Called while
/// they still exist (per scenario in suite mode; before teardown in single mode).
fn dump_artifacts(ctx: &Arc<Ctx>, logs: bool, save_audio: bool) {
    if !logs && !save_audio {
        return;
    }
    let sessions = ctx.sessions.lock().unwrap_or_else(|e| e.into_inner());
    if logs {
        dump_logs(&sessions);
    }
    if save_audio {
        save_recordings(&sessions);
    }
}

/// Tear all sessions down (hang up, let baresip flush BYEs, drop). Runs on the
/// runtime thread after the script finishes — the central counterpart to creating
/// agents.
fn teardown(ctx: &Arc<Ctx>, rt: &tokio::runtime::Runtime) {
    let sessions: Vec<AgentSession> = ctx
        .sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain()
        .map(|(_, s)| s)
        .collect();
    for s in &sessions {
        s.hangup_all();
    }
    rt.block_on(async { tokio::time::sleep(Duration::from_millis(200)).await });
    drop(sessions);
}

/// Print each agent's baresip log (SIP signaling) to stderr (`--logs`).
fn dump_logs(sessions: &HashMap<String, AgentSession>) {
    let mut names: Vec<&String> = sessions.keys().collect();
    names.sort();
    for name in names {
        eprintln!("\n── baresip log: {name} ──");
        match std::fs::read_to_string(sessions[name].log_path()) {
            Ok(content) => eprint!("{content}"),
            Err(e) => eprintln!(
                "(could not read {}: {e})",
                sessions[name].log_path().display()
            ),
        }
    }
}

/// Copy each agent's call recordings (sent `-enc` / received `-dec`) to the cwd,
/// named with a shared run timestamp (`--save-audio`).
fn save_recordings(sessions: &HashMap<String, AgentSession>) {
    let run_ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let mut names: Vec<&String> = sessions.keys().collect();
    names.sort();
    for name in names {
        let dir = sessions[name].recording_dir();
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut wavs: Vec<std::path::PathBuf> = entries
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("dump-") && n.ends_with(".wav"))
            })
            .collect();
        wavs.sort();
        for src in wavs {
            let dir_tag = if src.to_string_lossy().ends_with("-enc.wav") {
                "sent"
            } else {
                "recv"
            };
            let dst =
                std::path::PathBuf::from(format!("ringo-audio-{run_ts}-{name}-{dir_tag}.wav"));
            match std::fs::copy(&src, &dst) {
                Ok(_) => eprintln!("saved recording: {} ({name} {dir_tag})", dst.display()),
                Err(e) => eprintln!("(could not save {}: {e})", dst.display()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build_matcher;

    #[test]
    fn substring_is_default_and_case_insensitive() {
        let m = build_matcher("blind refer").unwrap();
        assert!(m("transfer (blind refer): target accepts"));
        assert!(!m("simple call"));
        // case-insensitive
        assert!(build_matcher("TRANSFER").unwrap()("transfer: rejects"));
        // parens are literal, not regex
        assert!(build_matcher("(blind refer)").unwrap()("x (blind refer) y"));
    }

    #[test]
    fn re_prefix_is_a_regex() {
        let m = build_matcher("re:^transfer").unwrap();
        assert!(m("transfer: accepts"));
        assert!(!m("a transfer")); // anchored
        assert!(build_matcher("re:tra(").is_err()); // invalid regex
    }
}
