//! Scripting frontends over the neutral [`crate::engine`]. Rhai is the production
//! frontend; `js` is an exploratory QuickJS spike (reached via `--lang js`). A new
//! language implements [`crate::engine::ScriptHost`] and gets its own submodule
//! here, reusing the same engine, verbs, assertions and runner.

pub(crate) mod dotenv;
pub mod js;
pub mod rhai;

/// Write the editor definition file for the selected frontend, introspected from the
/// registered bindings: rhai's `.d.rhai` from its function metadata, the JS `.d.ts`
/// from the `#[ts_global]`/`#[ts_export]`/`#[derive(Ts…)]` compile-time metadata.
pub fn write_definitions(lang: Lang, out: &Path) -> Result<()> {
    match lang {
        Lang::Rhai => self::rhai::write_definitions(out),
        Lang::Js => self::js::write_definitions(out),
    }
}

/// Write the generated API reference. Rhai renders one Markdown page per section into
/// the `out` directory. The JS API docs are generated from the hand-written `.d.ts` by
/// TypeDoc (outside the binary), not here.
pub fn write_book_api(lang: Lang, out: &Path) -> Result<()> {
    match lang {
        Lang::Rhai => self::rhai::write_book_api(out),
        Lang::Js => anyhow::bail!(
            "JS API docs are generated from the hand-written .d.ts via TypeDoc, not by this command"
        ),
    }
}

use crate::engine::Filters;
use crate::runtime::Output;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Scenario names for `--scenario` completion, dispatched by file extension
/// (`.js` → the QuickJS scanner, else the rhai AST scanner).
pub fn scenario_names(path: &Path) -> Vec<String> {
    if path.extension().is_some_and(|e| e == "js") {
        self::js::scenario_names(path)
    } else {
        self::rhai::scenario_names(path)
    }
}

/// Syntax-check a scenario file with the selected frontend.
pub fn check(lang: Lang, file: &Path) -> Result<()> {
    match lang {
        Lang::Rhai => self::rhai::check(file),
        Lang::Js => self::js::check(file),
    }
}

/// Which scripting frontend to use. Rhai is the default; `Js` selects the QuickJS
/// frontend. Usually inferred from the file extension (see [`detect_lang`]); the
/// `--lang` flag is an explicit override.
#[derive(Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Lang {
    #[default]
    Rhai,
    Js,
}

/// Infer the frontend from the scenario path(s) when `--lang` isn't given: a `.js`
/// file — or a directory that contains `.js` files — selects the JS frontend;
/// anything else stays on rhai.
pub fn detect_lang(paths: &[PathBuf]) -> Lang {
    if paths.iter().any(|p| contains_js(p)) { Lang::Js } else { Lang::Rhai }
}

/// Whether `path` is a `.js` file, or a directory holding one (recursively).
fn contains_js(path: &Path) -> bool {
    if path.extension().is_some_and(|e| e == "js") {
        return true;
    }
    path.is_dir()
        && std::fs::read_dir(path)
            .map(|entries| entries.flatten().any(|e| contains_js(&e.path())))
            .unwrap_or(false)
}

/// Dispatch `run` to the selected frontend. Both take the full feature set
/// (`--set` overrides, `--env-file` layering).
pub fn run(
    lang: Lang,
    paths: &[PathBuf],
    output: Output,
    overrides: HashMap<String, String>,
    filters: Filters,
    env_files: &[PathBuf],
) -> Result<()> {
    match lang {
        Lang::Rhai => self::rhai::run(paths, output, overrides, filters, env_files),
        Lang::Js => self::js::run(paths, output, overrides, filters, env_files),
    }
}
