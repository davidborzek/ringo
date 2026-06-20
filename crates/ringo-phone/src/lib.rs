// Backend, protocol and call engine live in the shared `ringo-core` crate.
// Re-export them so existing `crate::client`, `crate::baresip`, `crate::rlog!`
// paths keep resolving unchanged.
pub use ringo_core::rlog;
pub use ringo_core::{baresip, client, event, log, phone};

pub mod app;
pub mod config;
pub mod contacts;
pub mod control;
pub mod form;
pub mod header;
pub mod history;
pub mod hooks;
pub mod notify;
pub mod picker;
pub mod profile;
pub mod tui;
