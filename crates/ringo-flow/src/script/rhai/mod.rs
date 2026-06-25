//! Rhai scripting frontend: compiles `.rhai` scenarios and drives them through
//! the neutral [`crate::engine`] via [`RhaiHost`](host::RhaiHost). The whole
//! script runs on a `spawn_blocking` thread, so verbs may `block_on`; assertions
//! are value-based (`expected … but was …`), so they read well even from
//! imported modules.

/// Register a native function with metadata so it shows up in the generated
/// `.d.rhai` and the scenario API reference with named parameters, clean Rhai
/// types and a doc-comment. `params` is `["name: type", …, "ReturnType"]` (last).
/// Defined before the submodules so they can use it.
macro_rules! reg {
    ($engine:expr, $name:expr, [$($pi:literal),* $(,)?], $doc:expr, $func:expr $(,)?) => {
        rhai::FuncRegistration::new($name)
            .with_params_info([$($pi),*])
            .with_comments([$doc])
            .register_into_engine($engine, $func);
    };
}

mod bindings;
mod convert;
mod docs;
mod host;
mod types;

pub use docs::{write_book_api, write_definitions};
pub use host::scenario_names;

use crate::engine::{self, Ctx};
use crate::runtime::Output;
use anyhow::{Context, Result, anyhow, bail};
use rhai::Engine;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Built-in `await_until` timeout when neither the scenario (`default_timeout(…)`)
/// nor a per-call argument sets one.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Run scenario files. Each `path` is a `.rhai` file or a directory (expanded to
/// its `*.rhai` files, recursively). Each file is its own program — own engine,
/// own `setup`/`teardown`, own scenarios — run in sequence with sessions reset
/// between them. `overrides` (`--set key=value`) apply to every file.
pub fn run(
    paths: &[PathBuf],
    output: Output,
    overrides: HashMap<String, String>,
    filters: engine::Filters,
    env_files: &[PathBuf],
) -> Result<()> {
    let files = collect_files(paths)?;

    // Shared env from `--env-file` (later files win); a per-file `<scenario>.env`
    // is layered on top in each build closure, so `env(...)` resolves
    // per-file → shared → process.
    let shared_env = load_env_files(env_files)?;

    // One build closure per file (same closure type → homogeneous Vec). It runs
    // on the blocking thread: read + compile the file (syntax errors surface as
    // its build error), wire `import` resolution to the file's dir, and hand the
    // engine + AST to the registry so `parallel` can run closures on threads.
    let programs: Vec<(String, _)> = files
        .into_iter()
        .map(|path| {
            let overrides = overrides.clone();
            // Per-file env = shared + sibling `<stem>.env` (the latter wins).
            let mut env = shared_env.clone();
            let sibling = path.with_extension("env");
            let sibling = if sibling != path && sibling.is_file() {
                Some(sibling)
            } else {
                None
            };
            let label = path.display().to_string();
            let build = move |ctx: Arc<Ctx>| -> Result<host::RhaiHost> {
                if let Some(sibling) = &sibling {
                    merge_dotenv(sibling, &mut env)?;
                }
                // Mutable so `load_env(...)` can add more at run time; `env(...)`
                // reads it under the lock.
                let env = Arc::new(std::sync::Mutex::new(env));
                // `import` and `load_env` resolve relative to the scenario's dir.
                let base = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
                let source = std::fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?;
                let registry = Arc::new(host::Registry::default());
                let mut engine = bindings::build_engine(ctx, registry.clone(), env, base.clone());
                engine.set_module_resolver(
                    rhai::module_resolvers::FileModuleResolver::new_with_path(base),
                );
                let ast = engine
                    .compile(&source)
                    .map_err(|e| anyhow!("in {}: {e}", path.display()))?;
                let engine = Arc::new(engine);
                let ast = Arc::new(ast);
                registry.set_exec(Arc::downgrade(&engine), ast.clone());
                Ok(host::RhaiHost::new(engine, ast, registry, overrides))
            };
            (label, build)
        })
        .collect();

    engine::run(programs, output, DEFAULT_TIMEOUT, filters)
}

/// Expand the given paths into `.rhai` files: a file is taken as-is, a directory
/// is walked recursively for `*.rhai`. Results are sorted (stable run order) and
/// de-duplicated; an empty result is an error.
fn collect_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            // A directory yields its scenario files; helper/module files (no
            // `scenario(...)`) are skipped so they don't log as empty passes.
            let mut found = Vec::new();
            collect_dir(p, &mut found, 0)
                .with_context(|| format!("scan directory {}", p.display()))?;
            found.retain(|f| host::dir_should_run(f));
            files.extend(found);
        } else {
            files.push(p.clone()); // explicitly named → always run
        }
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        bail!("no .rhai scenario files found");
    }
    Ok(files)
}

/// Load and merge several `--env-file`s into one map (later files win).
fn load_env_files(paths: &[PathBuf]) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    for p in paths {
        merge_dotenv(p, &mut env)?;
    }
    Ok(env)
}

/// Parse a dotenv file (`KEY=VALUE` per line; `#` comments and blank lines
/// ignored; optional `export ` prefix; optional surrounding quotes) and merge it
/// into `env`, overwriting existing keys.
fn merge_dotenv(path: &Path, env: &mut HashMap<String, String>) -> Result<()> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read env file {}", path.display()))?;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, value) = line
            .split_once('=')
            .with_context(|| format!("{}:{}: expected KEY=VALUE", path.display(), i + 1))?;
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        env.insert(key.trim().to_string(), value.to_string());
    }
    Ok(())
}

/// How deep `collect_dir` recurses, to bound work and break symlink cycles.
const MAX_DIR_DEPTH: usize = 32;

/// Recursively collect `*.rhai` files under `dir`. Symlinks are not followed (so a
/// symlink loop can't trap us) and recursion is depth-capped as a backstop.
fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> std::io::Result<()> {
    if depth >= MAX_DIR_DEPTH {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        // Don't traverse symlinks (cycle/escape safety); `file_type()` doesn't
        // follow them, unlike `Path::is_dir`.
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            collect_dir(&path, out, depth + 1)?;
        } else if path.extension().is_some_and(|e| e == "rhai") {
            out.push(path);
        }
    }
    Ok(())
}

/// Syntax-check a scenario without running it (no baresip): `compile()` surfaces
/// parse errors with a position. Function/argument errors are dynamic in Rhai and
/// only show at run time, so this is a syntax gate, not full validation.
pub fn check(path: &Path) -> Result<()> {
    let src = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Engine::new()
        .compile(&src)
        .map_err(|e| anyhow!("in {}: {e}", path.display()))?;
    println!("{}: syntax ok", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::merge_dotenv;

    #[test]
    fn dotenv_parses_comments_quotes_export_and_overrides() {
        let path = std::env::temp_dir().join("ringo_flow_dotenv_test.env");
        std::fs::write(
            &path,
            "# a comment\n\
             \n\
             RF_USER=alice\n\
             export RF_PASS=\"s e cret\"\n\
             RF_DOM='example.com'\n\
             RF_USER=bob\n", // later line overrides
        )
        .unwrap();
        let mut env = std::collections::HashMap::new();
        env.insert("KEEP".into(), "yes".into());
        merge_dotenv(&path, &mut env).unwrap();
        assert_eq!(env["RF_USER"], "bob"); // last wins
        assert_eq!(env["RF_PASS"], "s e cret"); // export + double quotes stripped
        assert_eq!(env["RF_DOM"], "example.com"); // single quotes stripped
        assert_eq!(env["KEEP"], "yes"); // pre-existing keys kept
    }
}
