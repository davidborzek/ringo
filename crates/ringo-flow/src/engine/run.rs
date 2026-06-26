//! The language-neutral runner: a [`ScriptHost`] plugs a scripting language into
//! the engine. `run` owns the runtime, the per-scenario isolation, teardown and
//! reporting; the host only knows how to evaluate the top-level pass and a single
//! scenario. Adding a language means implementing [`ScriptHost`] — nothing here
//! changes.

use super::ctx::Ctx;
use crate::runtime::Output;
use crate::runtime::audio;
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
    /// Run one named scenario in isolation (`setup` → body → `teardown`). A scenario
    /// may also skip itself at runtime (e.g. env-gated `skip("reason")`).
    fn run_scenario(&mut self, name: &str) -> ScenarioResult;
}

/// A registered scenario plus its declared metadata (`tags` / `skip` / `only`),
/// produced by the top-level pass and used by [`run`] to select and report it.
#[derive(Clone, Default)]
pub struct ScenarioInfo {
    pub name: String,
    pub tags: Vec<String>,
    /// Statically disabled (`#{ skip: true }` / `#{ skip: "reason" }`).
    pub skip: bool,
    pub skip_reason: Option<String>,
    /// Focused (`#{ only: true }`): if any scenario sets it, only those run.
    pub only: bool,
}

/// Outcome of running a single scenario.
pub enum ScenarioResult {
    Passed,
    /// Skipped at runtime via `skip(...)` (distinct from a statically-skipped one).
    Skipped(Option<String>),
    Failed(String),
}

/// Result of the top-level pass.
pub enum TopLevel {
    /// No scenarios registered → the top-level code itself was the scenario.
    Single(std::result::Result<(), String>),
    /// Scenarios were registered; `top_error` carries a top-level pass failure.
    Suite {
        scenarios: Vec<ScenarioInfo>,
        top_error: Option<String>,
    },
}

/// Which scenarios to run: `--scenario` name pattern plus tag include/exclude.
#[derive(Default)]
pub struct Filters {
    /// `--scenario`: name substring, or `re:<regex>`.
    pub name: Option<String>,
    /// `--tag`: run only scenarios carrying at least one of these tags.
    pub tags: Vec<String>,
    /// `--exclude-tag`: skip scenarios carrying any of these tags.
    pub exclude_tags: Vec<String>,
}

impl Filters {
    fn is_active(&self) -> bool {
        self.name.is_some() || !self.tags.is_empty() || !self.exclude_tags.is_empty()
    }
}

/// A compiled selection: the name matcher plus the tag include/exclude lists.
struct Selector {
    matcher: Option<ScenarioMatcher>,
    include_tags: Vec<String>,
    exclude_tags: Vec<String>,
}

impl Selector {
    /// Whether `info` is selected to run: name matches (if a pattern was given), it
    /// carries an included tag (if `--tag` was given), and none of its tags are
    /// excluded.
    fn selects(&self, info: &ScenarioInfo) -> bool {
        self.matcher.as_ref().is_none_or(|m| m(&info.name))
            && (self.include_tags.is_empty()
                || info.tags.iter().any(|t| self.include_tags.contains(t)))
            && !info.tags.iter().any(|t| self.exclude_tags.contains(t))
    }
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
    filters: Filters,
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
    let matcher = filters.name.as_deref().map(build_matcher).transpose()?;
    let selector = Selector {
        matcher,
        include_tags: filters.tags.clone(),
        exclude_tags: filters.exclude_tags.clone(),
    };

    let ctx = Arc::new(Ctx::new(rt.handle().clone(), reporter, default_timeout));
    ctx.set_save_audio(output.save_audio);
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
            run_files(programs, &c, &selector, logs, save_audio, multi)
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
    if filters.is_active() && agg.scenarios == 0 {
        return Err(anyhow!(
            "no scenario matched the given filters (--scenario / --tag / --exclude-tag)"
        ));
    }

    if multi {
        ctx.emit(&Event::RunFinished {
            files: agg.files,
            passed_files: agg.passed_files,
            scenarios: agg.scenarios,
            passed_scenarios: agg.passed_scenarios,
            skipped_scenarios: agg.skipped_scenarios,
        });
    }

    // Shut down the tokio runtime explicitly with a timeout — background tasks
    // (event reader, header poll) may still be running and would block the
    // runtime drop indefinitely.
    rt.shutdown_timeout(Duration::from_millis(500));

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
    skipped_scenarios: usize,
}

/// A suite file deferred to phase 2: its host (top-level pass already done, so the
/// scenarios are registered) and the scenario metadata read from it.
struct Deferred<H> {
    label: String,
    host: H,
    scenarios: Vec<ScenarioInfo>,
}

/// Run all programs (files). Done in two phases so `only` focus applies across the
/// **whole run**, not just within one file: phase 1 builds each file and runs its
/// top-level pass (single-scenario files and build/top-level errors complete here,
/// in order); phase 2 runs the suite files' scenarios, with any `only` focus seen
/// anywhere applied globally. A `FileStarted` header is emitted when running more
/// than one file.
///
/// Knowing the run-wide focus requires every top-level pass first, and a
/// single-scenario file *executes* during that pass — so when single-scenario and
/// suite files are listed together explicitly, the single-scenario ones report
/// before the deferred suites. Directory runs only contain suite files, so they
/// stay in order.
fn run_files<H, F>(
    programs: Vec<(String, F)>,
    ctx: &Arc<Ctx>,
    selector: &Selector,
    logs: bool,
    save_audio: bool,
    multi: bool,
) -> Aggregate
where
    H: ScriptHost,
    F: FnOnce(Arc<Ctx>) -> Result<H>,
{
    let mut agg = Aggregate::default();
    let mut deferred: Vec<Deferred<H>> = Vec::new();
    let mut focus = false;

    // ── Phase 1: build + top-level pass, in order ──
    for (label, build) in programs {
        agg.files += 1;
        let mut host = match build(ctx.clone()) {
            Ok(h) => h,
            Err(e) => {
                // A compile/build error fails just this file; keep going.
                if multi {
                    ctx.emit(&Event::FileStarted { path: &label });
                }
                ctx.emit(&Event::Finished {
                    passed: false,
                    error: Some(format!("{e}")),
                });
                continue;
            }
        };
        match host.run_top_level() {
            // No `scenario(...)` calls: the top-level code itself ran as the
            // scenario. It executed just now, so finish it here (in order).
            TopLevel::Single(r) => {
                if multi {
                    ctx.emit(&Event::FileStarted { path: &label });
                }
                dump_artifacts(ctx, logs, save_audio);
                ctx.reset_sessions();
                let ok = r.is_ok();
                ctx.emit(&Event::Finished {
                    passed: ok,
                    error: r.err(),
                });
                agg.scenarios += 1;
                agg.passed_scenarios += usize::from(ok);
                if ok {
                    agg.passed_files += 1;
                }
            }
            // A top-level pass error fails the file with no scenarios.
            TopLevel::Suite {
                top_error: Some(e), ..
            } => {
                if multi {
                    ctx.emit(&Event::FileStarted { path: &label });
                }
                ctx.emit(&Event::Finished {
                    passed: false,
                    error: Some(e),
                });
            }
            // A suite: defer its scenarios so `only` focus can span the whole run.
            TopLevel::Suite { scenarios, .. } => {
                focus |= scenarios.iter().any(|s| s.only);
                deferred.push(Deferred {
                    label,
                    host,
                    scenarios,
                });
            }
        }
    }

    // ── Phase 2: run the suite files' scenarios, with global `only` focus ──
    if focus {
        eprintln!("⚠ ringo-flow: `only` focus is active — running only the focused scenario(s)");
    }
    for mut d in deferred {
        if multi {
            ctx.emit(&Event::FileStarted { path: &d.label });
        }
        let (total, passed, skipped) = run_suite(
            &mut d.host,
            &d.scenarios,
            ctx,
            selector,
            focus,
            logs,
            save_audio,
        );
        ctx.reset_sessions(); // isolate the next file (fresh agents)
        ctx.emit(&Event::SuiteFinished {
            total,
            passed,
            skipped,
        });
        agg.scenarios += total;
        agg.passed_scenarios += passed;
        agg.skipped_scenarios += skipped;
        // A file passes when nothing failed — skipped scenarios don't count against it.
        if passed + skipped == total {
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

/// Run a suite file's registered scenarios in isolation (fresh agents between
/// scenarios via [`Ctx::reset_sessions`]), applying the selection filters and the
/// run-wide `only` focus. `--logs`/`--save-audio` are dumped per scenario while its
/// sessions still exist. Returns `(total, passed, skipped)`.
fn run_suite<H: ScriptHost>(
    host: &mut H,
    scenarios: &[ScenarioInfo],
    ctx: &Arc<Ctx>,
    selector: &Selector,
    focus: bool,
    logs: bool,
    save_audio: bool,
) -> (usize, usize, usize) {
    let (mut total, mut passed, mut skipped) = (0, 0, 0);
    for info in scenarios {
        // Selection (not counted): name/tag filters and `only` focus narrow the set.
        if !selector.selects(info) || (focus && !info.only) {
            continue;
        }
        total += 1;
        // Statically skipped: reported, never run.
        if info.skip {
            skipped += 1;
            ctx.emit(&Event::ScenarioSkipped {
                name: &info.name,
                reason: info.skip_reason.as_deref(),
            });
            continue;
        }
        ctx.emit(&Event::ScenarioStarted { name: &info.name });
        let result = host.run_scenario(&info.name);
        dump_artifacts(ctx, logs, save_audio); // before reset drops this scenario's sessions
        ctx.reset_sessions(); // isolation: fresh agents for the next scenario
        match result {
            ScenarioResult::Passed => {
                passed += 1;
                ctx.emit(&Event::ScenarioFinished {
                    name: &info.name,
                    passed: true,
                    error: None,
                });
            }
            ScenarioResult::Skipped(reason) => {
                skipped += 1;
                ctx.emit(&Event::ScenarioSkipped {
                    name: &info.name,
                    reason: reason.as_deref(),
                });
            }
            ScenarioResult::Failed(msg) => ctx.emit(&Event::ScenarioFinished {
                name: &info.name,
                passed: false,
                error: Some(msg),
            }),
        }
    }
    (total, passed, skipped)
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
    // Wait for all calls to hang up (BYE flush) before dropping sessions.
    rt.block_on(async {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            if ringo_core::call_count() == 0 {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
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

/// Write each agent's in-process captured audio (sent + received) to the cwd as
/// WAV, named with a shared run timestamp (`--save-audio`). The backend captures
/// the audio in memory (no sndfile), so we serialise it ourselves.
fn save_recordings(sessions: &HashMap<String, AgentSession>) {
    let run_ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let mut names: Vec<&String> = sessions.keys().collect();
    names.sort();
    for name in names {
        let session = &sessions[name];
        for (tag, audio) in [
            ("sent", session.sent_audio()),
            ("recv", session.received_audio()),
        ] {
            let Some((samples, srate)) = audio else {
                continue;
            };
            if samples.is_empty() {
                continue;
            }
            let dst = std::path::PathBuf::from(format!("ringo-audio-{run_ts}-{name}-{tag}.wav"));
            match audio::write_wav(&dst, &samples, srate) {
                Ok(()) => eprintln!("saved recording: {} ({name} {tag})", dst.display()),
                Err(e) => eprintln!("(could not save {}: {e})", dst.display()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ScenarioInfo, Selector, build_matcher};

    fn info(name: &str, tags: &[&str]) -> ScenarioInfo {
        ScenarioInfo {
            name: name.to_string(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn selector(name: Option<&str>, include: &[&str], exclude: &[&str]) -> Selector {
        Selector {
            matcher: name.map(|n| build_matcher(n).unwrap()),
            include_tags: include.iter().map(|s| s.to_string()).collect(),
            exclude_tags: exclude.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn empty_selector_selects_everything() {
        let s = selector(None, &[], &[]);
        assert!(s.selects(&info("anything", &[])));
        assert!(s.selects(&info("tagged", &["x"])));
    }

    #[test]
    fn tag_include_requires_one_match() {
        let s = selector(None, &["smoke"], &[]);
        assert!(s.selects(&info("a", &["smoke", "fast"])));
        assert!(!s.selects(&info("b", &["slow"])));
        assert!(!s.selects(&info("c", &[]))); // untagged excluded when --tag given
    }

    #[test]
    fn tag_exclude_wins_over_include() {
        let s = selector(None, &["smoke"], &["slow"]);
        assert!(s.selects(&info("a", &["smoke"])));
        // carries an excluded tag → out, even though it also has an included one
        assert!(!s.selects(&info("b", &["smoke", "slow"])));
    }

    #[test]
    fn name_and_tags_combine() {
        let s = selector(Some("call"), &["smoke"], &[]);
        assert!(s.selects(&info("answered call", &["smoke"])));
        assert!(!s.selects(&info("answered call", &["slow"]))); // name ok, tag no
        assert!(!s.selects(&info("registration", &["smoke"]))); // tag ok, name no
    }

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
