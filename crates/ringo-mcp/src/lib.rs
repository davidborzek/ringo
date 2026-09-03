//! `ringo-mcp` — an MCP server over stdio that exposes ringo SIP agents
//! (user agents) to LLM agents for telephony.
//!
//! Architecture: one `ringo-agent` worker *process* per configured account
//! (each worker is one baresip UA, driven over its framed stdio protocol),
//! managed by a [`hub::Hub`]. Agents start **lazily** — the hub spawns a
//! worker on the first tool call that touches an agent (single-flight), and a
//! dead worker is respawned the same way. The MCP tool surface in [`server`]
//! is a thin layer over the per-agent [`ringo_agent::ProcessClient`] handles.
//!
//! The binary re-invokes itself as `ringo-mcp agent` for the worker processes
//! (see `ProcessClient::spawn`, which spawns `current_exe` with the `agent`
//! argument), so this crate is both the MCP server and the worker host.

#![warn(missing_docs)]

mod bridge;
pub mod config;
mod headers;
pub mod hub;
mod server;
mod state;

pub use server::{event_name, resolve_disabled_tools, serve, total_tool_count};
