//! `ringo-mcp` — MCP server binary. See `lib.rs` for the architecture.
//!
//! Subcommands:
//! - *(none)* — load the config, spawn the agents, serve MCP on stdio.
//! - `agent` — internal: one worker process, driven by the framed stdio
//!   protocol (the MCP server spawns *itself* with this argument; not for
//!   direct use).

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ringo_mcp::{config, hub, serve};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "ringo-mcp",
    version,
    about = "MCP server (stdio) exposing ringo SIP agents",
    long_about = "An MCP server over stdio that gives LLM agents a telephone: SIP agents \
                  configured in a TOML file, driven from any MCP client. See the crate README \
                  for the config format and the full tool reference."
)]
struct Cli {
    /// Config file (default: $RINGO_MCP_CONFIG, or
    /// ~/.config/ringo-mcp/config.toml)
    #[arg(long, value_name = "PATH", global = true)]
    config: Option<PathBuf>,

    /// Comma-separated allowlist of tool groups and/or tool names —
    /// everything else is disabled
    #[arg(long, value_name = "LIST", value_delimiter = ',')]
    enabled_tools: Option<Vec<String>>,

    /// Disable a tool group or a single tool (repeatable; applied on top of
    /// --enabled-tools). Groups: discovery, call-control, audio, headers,
    /// events, streams, recording, lifecycle
    #[arg(long = "disable", value_name = "GROUP|TOOL", value_delimiter = ',')]
    disable: Vec<String>,

    /// Live-audio WS bridge bind host (loopback only; the port is always
    /// ephemeral)
    #[arg(long, value_name = "IP", default_value = "127.0.0.1")]
    bridge_host: IpAddr,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Internal: run a single agent as a worker process over the framed stdio
    /// protocol (spawned by the server as `<exe> agent`; not for direct use).
    #[command(hide = true)]
    Agent,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Internal worker process — must run before anything else touches config
    // or the runtime.
    if matches!(cli.command, Some(Commands::Agent)) {
        return ringo_agent::worker::run();
    }

    // Tool surface: validated up front (a typo'd flag must fail loudly).
    let disabled_tools =
        ringo_mcp::resolve_disabled_tools(cli.enabled_tools.as_deref(), &cli.disable)?;
    if !cli.bridge_host.is_loopback() {
        anyhow::bail!(
            "--bridge-host `{}` is not a loopback address",
            cli.bridge_host
        );
    }

    let path = cli
        .config
        .clone()
        .or_else(|| std::env::var_os("RINGO_MCP_CONFIG").map(PathBuf::from))
        .unwrap_or_else(config::default_path);
    let loaded = config::load(&path)?;

    // MCP stdio + broadcast/timers are async; the agent workers themselves run
    // as child processes (their own blocking loops), so a multi-thread runtime
    // only serves the server's I/O here.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    let n = loaded.agents.len();
    let hub = Arc::new(hub::Hub::new(loaded, cli.bridge_host));
    eprintln!(
        "ringo-mcp: {n} agent(s) configured (started on first use); {} tool(s) enabled",
        ringo_mcp::total_tool_count() - disabled_tools.len()
    );
    rt.block_on(async { serve(Arc::clone(&hub), disabled_tools).await })?;

    // The client is gone: ask the workers to deregister and exit, then give
    // the runtime a bounded window to drain — the per-agent event-poll tasks
    // only end once the workers' streams close, so a plain `drop(rt)` could
    // wait forever and a hard kill would skip the de-REGISTER. The grace
    // exceeds the worker's shutdown budget (de-REGISTER wait + RE-thread
    // stop, see ringo-agent's SHUTDOWN_GRACE).
    hub.shutdown();
    rt.shutdown_timeout(std::time::Duration::from_secs(35));
    Ok(())
}
