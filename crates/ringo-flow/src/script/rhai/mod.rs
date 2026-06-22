//! Rhai scripting frontend: compiles `.rhai` scenarios and drives them through
//! the neutral [`crate::engine`] via [`RhaiHost`](host::RhaiHost). The whole
//! script runs on a `spawn_blocking` thread, so verbs may `block_on`; assertions
//! are value-based (`expected … but was …`), so they read well even from
//! imported modules.

/// Register a native function with metadata so it shows up in the generated
/// `.d.rhai` (and HTML docs) with named parameters, clean Rhai types and a
/// doc-comment. `params` is `["name: type", …, "ReturnType"]` (return type last).
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
mod host;
mod types;

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

/// Build an engine purely to enumerate the registered API (definitions/docs). No
/// baresip is started; the throwaway `Ctx`'s verbs are never called. The runtime
/// is returned so its `Handle` (held by `Ctx`) stays valid.
fn doc_engine() -> Result<(tokio::runtime::Runtime, Engine)> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let ctx = Arc::new(Ctx::new(
        rt.handle().clone(),
        Box::new(crate::runtime::report::Json),
        DEFAULT_TIMEOUT,
    ));
    let engine = bindings::build_engine(
        ctx,
        Arc::new(host::Registry::default()),
        Arc::default(),
        PathBuf::from("."),
    );
    Ok((rt, engine))
}

/// Write a Rhai definition file (`.d.rhai`) describing the whole scenario API
/// (functions, getters, types, the `State` namespace) for the Rhai language
/// server (completion/hover).
pub fn write_definitions(out: &Path) -> Result<()> {
    let (_rt, engine) = doc_engine()?;
    let scope = rhai::Scope::new();
    engine
        .definitions_with_scope(&scope)
        .write_to_file(out)
        .with_context(|| format!("write {}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

/// The documented scenario API as `(signature label, doc lines)`, sorted by
/// label. Operators and the Rhai stdlib (no doc comments) are filtered out, so
/// this is exactly what the `reg!` calls document — the single source for both
/// the HTML and the Markdown reference.
fn api_functions(engine: &Engine) -> Result<Vec<(String, Vec<String>)>> {
    let json = engine
        .gen_fn_metadata_to_json(false)
        .context("generate function metadata")?;
    let meta: serde_json::Value = serde_json::from_str(&json).context("parse metadata JSON")?;
    let mut out: Vec<(String, Vec<String>)> = meta["functions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let docs: Vec<String> = f["docComments"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|d| d.as_str())
                .flat_map(clean_doc)
                .collect();
            (!docs.is_empty()).then(|| (sig_label(f), docs))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// The reference section a signature belongs to, for the grouped Markdown. This
/// is presentation-only: group by the receiver type (first parameter), with the
/// audio verbs split out by name. Returns `(order, title)`.
fn category(label: &str) -> (u8, &'static str) {
    let name = label
        .trim_start_matches("get ")
        .split('(')
        .next()
        .unwrap_or("");
    if matches!(
        name,
        "send_audio" | "verify_audio" | "verify_audio_connection" | "tone" | "file" | "silent"
    ) {
        (5, "Audio")
    } else if label.contains("Assertion") {
        (2, "Assertions & matchers")
    } else if label.contains("HttpMock")
        || label.contains("MockRequest")
        || matches!(name, "mock_server" | "json_response" | "text_response")
    {
        (4, "HTTP mock server")
    } else if label.contains("HttpResponse") {
        (3, "HTTP")
    } else if label.contains("Agent") || label.contains("Peer") {
        (1, "Agents")
    } else {
        (0, "Top-level")
    }
}

/// Render the scenario API as Markdown, grouped into sections. Pure (no I/O) so a
/// test can compare it against the committed `docs/scenario-api.md`.
fn render_markdown_docs(engine: &Engine) -> Result<String> {
    // `funcs` is already sorted by label, so within each group the entries stay
    // alphabetical; the BTreeMap key `(order, title)` orders the sections.
    let mut groups: std::collections::BTreeMap<(u8, &'static str), String> =
        std::collections::BTreeMap::new();
    for (label, docs) in api_functions(engine)? {
        let body = groups.entry(category(&label)).or_default();
        body.push_str(&format!("### `{label}`\n\n"));
        for line in docs {
            body.push_str(&line);
            body.push('\n');
        }
        body.push('\n');
    }

    let mut md = String::from(
        "# ringo-flow scenario API\n\n\
         Functions, getters and types available in a `.rhai` scenario. **Generated** \
         from the engine with `ringo-flow docs` — do not edit by hand; see the \
         [README](../README.md) for concepts and usage.\n\n",
    );
    for ((_, title), body) in &groups {
        md.push_str(&format!("## {title}\n\n{body}"));
    }
    Ok(md)
}

/// Write the Markdown scenario API reference (git-friendly, linkable).
pub fn write_markdown_docs(out: &Path) -> Result<()> {
    let (_rt, engine) = doc_engine()?;
    let md = render_markdown_docs(&engine)?;
    std::fs::write(out, md).with_context(|| format!("write {}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

/// Write an HTML reference of the scenario API, rendered from the engine's
/// function metadata (so it stays in sync). Only documented functions are shown.
pub fn write_html_docs(out: &Path) -> Result<()> {
    let (_rt, engine) = doc_engine()?;
    let mut items = String::new();
    for (label, docs) in api_functions(&engine)? {
        items.push_str(&format!(
            "<section><h3><code>{}</code></h3>\n",
            html_escape(&label)
        ));
        for line in docs {
            items.push_str(&format!("<p>{}</p>\n", html_escape(&line)));
        }
        items.push_str("</section>\n");
    }

    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>ringo-flow scenario API</title><style>\
body{{font:16px/1.5 system-ui,sans-serif;max-width:48rem;margin:2rem auto;padding:0 1rem;color:#222}}\
h1{{border-bottom:2px solid #eee;padding-bottom:.3rem}}\
section{{border-top:1px solid #eee;padding:.3rem 0}}\
code{{background:#f5f5f5;padding:.1rem .3rem;border-radius:3px;font-size:.95em}}\
h3{{margin:.6rem 0 .2rem}} p{{margin:.2rem 0}}\
</style></head><body>\
<h1>ringo-flow scenario API</h1>\
<p>Functions and getters available in a <code>.rhai</code> scenario. Generated \
from the engine — see the README for usage.</p>\n{items}</body></html>\n"
    );
    std::fs::write(out, html).with_context(|| format!("write {}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

/// A readable signature; getters (`get$x(…)`) render as `get x(…)`.
fn sig_label(f: &serde_json::Value) -> String {
    let sig = f["signature"].as_str().unwrap_or("");
    match sig.strip_prefix("get$") {
        Some(rest) => format!("get {rest}"),
        None => sig.to_string(),
    }
}

/// Split a `///`/`/**` doc block into clean text lines.
fn clean_doc(block: &str) -> Vec<String> {
    block
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches("/**")
                .trim_end_matches("*/")
                .trim_start_matches("///")
                .trim_start_matches('*')
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::merge_dotenv;

    #[test]
    fn markdown_reference_is_current() {
        // The committed reference is generated; this fails if it drifts from the
        // engine's registered API so it can't go stale silently.
        let (_rt, engine) = super::doc_engine().unwrap();
        let generated = super::render_markdown_docs(&engine).unwrap();
        let committed = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/scenario-api.md"));
        assert_eq!(
            generated, committed,
            "docs/scenario-api.md is stale — regenerate with \
             `cargo run -p ringo-flow -- docs crates/ringo-flow/docs/scenario-api.md`"
        );
    }

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
