//! Scripting frontends over the neutral [`crate::engine`]. Currently Rhai; a new
//! language implements [`crate::engine::ScriptHost`] and gets its own submodule
//! here, reusing the same engine, verbs, assertions and runner.

pub mod rhai;

pub use self::rhai::{check, run, scenario_names, write_book_api, write_definitions};
