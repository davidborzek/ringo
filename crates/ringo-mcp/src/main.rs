//! `ringo-mcp` — MCP server binary. See `lib.rs` for the architecture.
//!
//! Subcommands:
//! - *(none)* / `serve` — load the config, spawn the agents, serve MCP on stdio.
//! - `agent` — internal: one worker process, driven by the framed stdio
//!   protocol (the MCP server spawns *itself* with this argument; not for
//!   direct use).

use anyhow::{Context, Result, bail};
use ringo_mcp::{config, hub, serve};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);

    // Internal worker process (spawned by ProcessClient as `<exe> agent`).
    if args.next().as_deref() == Some("agent") {
        return ringo_agent::worker::run();
    }
    // (`None` was consumed above only when there were no args at all.)
    let mut first = std::env::args().nth(1);
    if first.as_deref() == Some("serve") {
        first = std::env::args().nth(2);
    }

    let mut config_path = None;
    let mut rest = first.into_iter().chain(args);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--config" => config_path = Some(rest.next().context("--config expects a path")?),
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
    rt.block_on(async move {
        // Config was validated above (passwords resolved, names/fields checked);
        // the hub itself spawns nothing — agents start lazily on first tool use.
        let hub = hub::Hub::new(loaded);
        eprintln!("ringo-mcp: {n} agent(s) configured (started on first use)");
        serve(std::sync::Arc::new(hub)).await
    })
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
