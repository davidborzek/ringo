//! QuickJS (`rquickjs`) scripting frontend: compiles `.js` scenarios and drives
//! them through the neutral [`crate::engine`] via [`JsHost`](host::JsHost). The
//! whole script runs on a `spawn_blocking` thread (verbs `block_on`); QuickJS is
//! single-threaded/sync, which fits exactly. A minimal spike — see the crate-level
//! spike notes — covering enough of the DSL for one real scenario.

mod bindings;
mod convert;
mod dsl;
mod host;
mod tsgen;

/// Banner prepended to the generated `.d.ts`, explaining where it comes from.
const DTS_HEADER: &str = "\
// Type definitions for the ringo-flow scenario DSL (JS/TS frontend).
// GENERATED — do not edit. Interface methods + globals are derived from the Rust
// binding signatures (#[ts_export] / #[ts_global]); data-shape types from
// #[derive(TsInterface|TsEnum)] / declare!. Regenerate with
// `ringo-flow definitions --lang js`.";

/// Render and write the `.d.ts` for the JS frontend, derived from the bindings'
/// compile-time metadata (collected via `inventory`).
pub fn write_definitions(out: &std::path::Path) -> Result<()> {
    let dts = tsgen::dts(&tsgen::Api::collect(), DTS_HEADER);
    std::fs::write(out, dts).with_context(|| format!("write {}", out.display()))?;
    Ok(())
}

use crate::engine::{self, Ctx};
use crate::runtime::Output;
use crate::runtime::report::{Human, Level, Reporter};
use anyhow::{Context as _, Result, bail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Built-in `until` timeout when no per-call argument sets one.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Names registered via `scenario("name", …)` in a `.js` file, for `--scenario`
/// shell completion. A lightweight regex scan — no JS eval, so it never starts
/// baresip (mirrors how the rhai frontend scans the AST without evaluating).
pub fn scenario_names(path: &Path) -> Vec<String> {
    let Ok(src) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let re = regex::Regex::new(r#"\bscenario\s*\(\s*["']([^"']+)["']"#).expect("valid regex");
    re.captures_iter(&src).map(|c| c[1].to_string()).collect()
}

/// Syntax-check a `.js` scenario without running it (no baresip). Evals the
/// top-level against the full context: `scenario(...)` bodies are only registered
/// (not called), so a scenario file starts no agents. Reports parse/runtime
/// errors in the top-level pass.
pub fn check(path: &Path) -> Result<()> {
    let src = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let rt = tokio::runtime::Builder::new_current_thread().build()?;
    let reporter: Box<dyn Reporter + Send> = Box::new(Human::new(Level::Quiet));
    let ctx = Arc::new(Ctx::new(rt.handle().clone(), reporter, DEFAULT_TIMEOUT));
    let host = host::JsHost::new(
        ctx,
        src,
        path.display().to_string(),
        Arc::new(Mutex::new(HashMap::new())),
        HashMap::new(),
        base_dir(path),
    )?;
    let err = host.check_syntax().err();
    match err {
        None => {
            println!("{}: syntax ok", path.display());
            Ok(())
        }
        Some(e) => bail!("{e}"),
    }
}

/// Run `.js` scenario files. Each file is its own program (own runtime/context,
/// own scenarios), run in sequence with sessions reset between them. `overrides`
/// (`--set`) apply to every file; `env_files` (`--env-file`, later wins) plus a
/// per-file sibling `<stem>.env` feed `env(...)`.
pub fn run(
    paths: &[PathBuf],
    output: Output,
    overrides: HashMap<String, String>,
    filters: engine::Filters,
    env_files: &[PathBuf],
) -> Result<()> {
    let files = collect_files(paths)?;
    let shared_env = crate::script::dotenv::load_env_files(env_files)?;
    let programs: Vec<(String, _)> = files
        .into_iter()
        .map(|path| {
            let label = path.display().to_string();
            let label2 = label.clone();
            let overrides = overrides.clone();
            // Per-file env = shared `--env-file`s + sibling `<stem>.env` (latter wins).
            let mut env = shared_env.clone();
            let sibling = path.with_extension("env");
            let sibling = (sibling != path && sibling.is_file()).then_some(sibling);
            let build = move |ctx: Arc<Ctx>| -> Result<host::JsHost> {
                if let Some(sibling) = &sibling {
                    crate::script::dotenv::merge_dotenv(sibling, &mut env)?;
                }
                let source = std::fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?;
                host::JsHost::new(
                    ctx,
                    source,
                    label2.clone(),
                    Arc::new(Mutex::new(env)),
                    overrides,
                    base_dir(&path),
                )
            };
            (label, build)
        })
        .collect();
    engine::run(programs, output, DEFAULT_TIMEOUT, filters)
}

/// The directory `import`/`loadEnv` paths resolve against: the scenario file's
/// parent, or `.` when it has none.
fn base_dir(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// Expand paths into `.js` files (a directory is walked recursively).
fn collect_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            collect_dir(p, &mut files)?;
        } else {
            files.push(p.clone());
        }
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        bail!("no .js scenario files found");
    }
    Ok(files)
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_dir(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "js") {
            out.push(path);
        }
    }
    Ok(())
}
