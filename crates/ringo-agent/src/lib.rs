//! Run a SIP user agent as its own process, driven over a line-delimited JSON
//! (NDJSON) stdio protocol.
//!
//! A consumer drives an agent via [`ProcessClient`] (spawned with
//! [`ProcessClient::spawn`]), which forks a child that re-execs the host
//! binary's `agent` subcommand — the host wires that subcommand to
//! [`worker::run`]. Each agent is its own process, with its own SIP socket and
//! media stack, so several can run side by side without sharing the SIP stack's
//! process-global state — which a single shared process can't do once incoming
//! calls must each be routed to the right agent.
//!
//! Public surface: [`ProcessClient`] + [`AgentConfig`] to drive an agent,
//! [`worker::run`] (the worker entry the host re-execs), and [`audio`] (tone
//! analysis / WAV helpers). The NDJSON wire protocol is an internal detail.

#![warn(missing_docs)]

pub mod audio;
pub mod worker;
// The NDJSON wire protocol and the parent-side client are implementation
// details: consumers drive an agent through the re-exported `ProcessClient`
// (with `AgentConfig`), never the wire types directly.
pub(crate) mod client;
pub(crate) mod proto;

pub use client::ProcessClient;
pub use proto::AgentConfig;
