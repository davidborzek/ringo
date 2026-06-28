//! Logging setup for `serve`: a `tracing` subscriber with a configurable level
//! and either a human-readable or JSON format. Only `serve` uses this — the
//! `run`/`check` commands have their own reporters.

use anyhow::{Result, bail};
use tracing_subscriber::EnvFilter;

/// Initialise the global tracing subscriber. `level` is a tracing filter
/// (`trace`/`debug`/`info`/`warn`/`error`, or a full `RUST_LOG`-style directive);
/// `format` is `text` (default, human-readable) or `json`. `RUST_LOG`, if set,
/// overrides `level`.
pub fn init(level: &str, format: &str) -> Result<()> {
    // RUST_LOG wins for power users; otherwise use the --log-level value.
    let filter = match EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => EnvFilter::try_new(level)
            .map_err(|e| anyhow::anyhow!("invalid --log-level `{level}`: {e}"))?,
    };
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match format {
        "text" => builder.init(),
        "json" => builder.json().init(),
        other => bail!("invalid --log-format `{other}` (expected `text` or `json`)"),
    }
    Ok(())
}
