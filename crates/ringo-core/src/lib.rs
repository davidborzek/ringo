//! Shared engine for the ringo tools: backend abstraction and the phone
//! command interface. Kept free of any TUI or ringo-specific configuration so
//! it can back both the `ringo` softphone and the `ringo-flow` scenario runner.

pub mod account;
pub mod backend;
pub mod event;
pub mod log;
pub mod phone;

pub use backend::{
    AudioFrame, available_audio_codecs, call_count, is_registered, push_audio, received_audio,
    sent_audio, shutdown, sip_trace_file, sip_trace_stderr, start_audio_stream,
    subscribe_received_audio,
};

mod baresip;
