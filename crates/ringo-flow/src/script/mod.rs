//! Scripting frontends over the neutral [`crate::engine`]. Currently Rhai; a new
//! language implements [`crate::engine::ScriptHost`] and gets its own submodule
//! here, reusing the same engine, verbs, assertions and runner.

pub mod rhai;

pub use self::rhai::{
    check, run, scenario_names, write_definitions, write_html_docs, write_markdown_docs,
};
