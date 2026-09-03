//! `ringo-mcp` — MCP server binary. See `lib.rs` for the architecture.
//!
//! Subcommands:
//! - *(none)* / `serve` — load the config, spawn the agents, serve MCP on stdio.
//! - `agent` — internal: one worker process, driven by the framed stdio
//!   protocol (the MCP server spawns *itself* with this argument; not for
//!   direct use).

use anyhow::{Context, Result, bail};
use ringo_mcp::{config, hub, serve};
use std::sync::Arc;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);

    // Internal worker process (spawned by ProcessClient as `<exe> agent`).
    let first = args.next();
    if first.as_deref() == Some("agent") {
        return ringo_agent::worker::run();
    }
    // If argv[1] was `serve`, the options start at argv[2]; otherwise argv[1]
    // IS the first option — continue with the same iterator, once per arg.
    let mut next = if first.as_deref() == Some("serve") {
        args.next()
    } else {
        first
    };

    let mut config_path = None;
    while let Some(arg) = next {
        match arg.as_str() {
            "--config" => config_path = Some(args.next().context("--config expects a path")?),
            other if other.starts_with("--config=") => {
                config_path = Some(other.trim_start_matches("--config=").to_string())
            }
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => {
                print_usage();
                bail!("unexpected argument `{other}`");
            }
        }
        next = args.next();
    }

    let path = config_path
        .map(std::path::PathBuf::from)
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
    let hub = Arc::new(hub::Hub::new(loaded));
    eprintln!("ringo-mcp: {n} agent(s) configured (started on first use)");
    rt.block_on(async { serve(Arc::clone(&hub)).await })?;

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

fn print_usage() {
    println!(
        "ringo-mcp {} — MCP server (stdio) exposing ringo SIP agents\n\n\
         USAGE:\n  ringo-mcp [serve] [--config <PATH>]\n\n\
         Config defaults to $RINGO_MCP_CONFIG or ~/.config/ringo-mcp/config.toml.\n\
         See the crate README for the config format and available tools.",
        env!("CARGO_PKG_VERSION")
    );
}
