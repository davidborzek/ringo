//! Shared engine for the ringo tools: backend abstraction and the phone
//! command interface. Kept free of any TUI or ringo-specific configuration so
//! it can back both the `ringo` softphone and the `ringo-flow` scenario runner.

pub mod account;
pub mod backend;
pub mod event;
pub mod log;
pub mod phone;

pub use backend::{
    call_count, is_registered, received_audio, sent_audio, set_sip_trace, shutdown,
};

mod baresip;
