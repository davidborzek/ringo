//! `ringo-profile` — ringo's profile file format and custom-header templates.
//!
//! Shared by ringo-phone (the TUI softphone) and ringo-mcp (the MCP server):
//! loading, saving and resolving a `profile.toml`, plus the `${placeholder}`
//! templates used for outgoing custom SIP headers. FFI-free by design — the
//! baresip-backed crates build on top of this one, not the other way round.

pub mod header;
pub mod profile;
