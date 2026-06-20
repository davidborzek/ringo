//! Shared engine for the ringo tools: spawning baresip, the ctrl_tcp wire
//! protocol, the call-event model and the phone command abstraction. Kept free
//! of any TUI or ringo-specific configuration so it can back both the `ringo`
//! softphone and the `ringo-flow` scenario runner.

pub mod baresip;
pub mod client;
pub mod event;
pub mod log;
pub mod phone;
