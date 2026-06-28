mod engine;
mod runtime;
mod script;
#[cfg(feature = "server")]
mod serve;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::{
    engine::{ArgValueCandidates, CompletionCandidate},
    env::CompleteEnv,
};
use std::collections::HashMap;
use std::path::PathBuf;

/// Completion candidates for `--scenario`: the scenarios registered in the flow
/// file already on the command line. During completion the shell passes the words
/// being completed as our process args, so we recover the file from there and scan
/// it (no eval, so no baresip is started).
fn scenario_candidates() -> Vec<CompletionCandidate> {
    let args: Vec<String> = std::env::args().collect();
    let Some(file) = scenario_file_from_args(&args) else {
        return Vec::new();
    };
    script::scenario_names(&file)
        .into_iter()
        .map(CompletionCandidate::new)
        .collect()
}

/// The `run` positional (the scenario file) out of a partial command line: the
/// first non-flag word after `run`, skipping value-taking flags and their values.
fn scenario_file_from_args(args: &[String]) -> Option<PathBuf> {
    let mut rest = args.iter().skip_while(|a| *a != "run");
    rest.next()?; // consume "run"
    let mut rest = rest.peekable();
    while let Some(a) = rest.next() {
        match a.as_str() {
            // These take a separate value word; skip it so it isn't read as the file.
            "--set" | "--scenario" => {
                rest.next();
            }
            // `--flag` / `--flag=value` (and the lone `--` separator): not the file.
            _ if a.starts_with('-') => {}
            _ => return Some(PathBuf::from(a)),
        }
    }
    None
}

#[derive(Parser)]
#[command(
    name = "ringo-flow",
    version,
    about = "Telephony scenario test runner for baresip (Rhai scenarios)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a scenario file
    Run {
        /// Scenario files and/or directories (a directory runs its `*.rhai`,
        /// recursively)
        #[arg(required = true, num_args = 1.., value_hint = ValueHint::AnyPath)]
        paths: Vec<PathBuf>,
        /// Override a variable (repeatable): --set key=value
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// Load env vars from a dotenv file for `env(...)` (repeatable; later wins).
        /// A sibling `<scenario>.env` is layered on top, per file.
        #[arg(long = "env-file", value_name = "FILE", value_hint = ValueHint::FilePath)]
        env_file: Vec<PathBuf>,
        /// Run only scenarios whose name contains this (case-insensitive); prefix
        /// with `re:` for a regex, e.g. `re:^transfer`
        #[arg(long = "scenario", value_name = "PATTERN", add = ArgValueCandidates::new(scenario_candidates))]
        scenario: Option<String>,
        /// Run only scenarios carrying one of these tags (repeatable; comma-separated)
        #[arg(long = "tag", value_name = "TAG", value_delimiter = ',')]
        tag: Vec<String>,
        /// Skip scenarios carrying any of these tags (repeatable; comma-separated)
        #[arg(long = "exclude-tag", value_name = "TAG", value_delimiter = ',')]
        exclude_tag: Vec<String>,
        /// Write the backend log (SIP signaling etc.). Off by default — nothing
        /// is written. `--log` → stderr; `--log <FILE>` → that file.
        #[arg(long, value_name = "FILE", num_args = 0..=1)]
        log: Option<Option<PathBuf>>,
        /// Trace every SIP request/response to its own destination (separate from
        /// `--log`). Off by default. `--sip-trace` → stderr; `--sip-trace <FILE>`
        /// → that file.
        #[arg(long = "sip-trace", value_name = "FILE", num_args = 0..=1)]
        sip_trace: Option<Option<PathBuf>>,
        /// Save each agent's call recordings (sent/received WAV) to the cwd
        #[arg(long)]
        save_audio: bool,
        /// Emit machine-readable NDJSON events instead of the human log
        #[arg(long)]
        json: bool,
        /// Print per-agent media-quality metrics (MOS/jitter/loss) at each
        /// scenario's end: a compact human line on its own, or `metric` NDJSON
        /// events when combined with `--json` (what `serve` reads).
        #[arg(long)]
        metrics: bool,
        /// More detail (shows observed state on every assertion)
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,
        /// Only print failures and the final result
        #[arg(short, long)]
        quiet: bool,
        /// Disable ANSI colors/bold in the human log (also honors `NO_COLOR`)
        #[arg(long, visible_alias = "no-ansi")]
        no_color: bool,
        /// Skip TLS certificate verification for `http(...)` (DANGER; also via
        /// `RINGO_FLOW_INSECURE_HTTP`). Prefer mounting the CA — see the README.
        #[arg(long)]
        insecure_http: bool,
    },
    /// Syntax-check a scenario without running it (no baresip)
    Check {
        /// Path to the .rhai scenario
        #[arg(value_hint = ValueHint::FilePath)]
        file: PathBuf,
    },
    /// Write a Rhai definition file (.d.rhai) for editor completion/hover
    Definitions {
        /// Output path
        #[arg(default_value = "docs/src/ringo-flow/ringo-flow.d.rhai", value_hint = ValueHint::FilePath)]
        out: PathBuf,
    },
    /// Generate the scenario API reference: one Markdown page per section into the
    /// given directory (the mdBook `src/api`).
    Docs {
        /// Output directory for the generated API pages
        #[arg(default_value = "docs/src/ringo-flow/api", value_hint = ValueHint::DirPath)]
        out: PathBuf,
    },
    /// Run as a monitor: scheduled scenario runs + Prometheus metrics over HTTP
    #[cfg(feature = "server")]
    Serve {
        /// Path to the `monitor.toml` config
        #[arg(value_hint = ValueHint::FilePath)]
        config: PathBuf,
        /// Override the listen address (host:port)
        #[arg(long, env = "RINGO_FLOW_SERVE_LISTEN")]
        listen: Option<String>,
        /// Override just the listen port (keeps the host); wins over --listen
        #[arg(long, env = "RINGO_FLOW_SERVE_PORT")]
        port: Option<u16>,
        /// Override the default per-run timeout (e.g. `120s`)
        #[arg(long, env = "RINGO_FLOW_SERVE_TIMEOUT")]
        timeout: Option<String>,
        /// Run the cron schedulers (default `true`); `false` serves the HTTP API
        /// without firing any schedules
        #[arg(long, env = "RINGO_FLOW_SERVE_SCHEDULER")]
        scheduler: Option<bool>,
        /// Enable (`true`) or disable (`false`) the `/metrics` endpoint
        #[arg(long, env = "RINGO_FLOW_SERVE_METRICS")]
        metrics: Option<bool>,
        /// Override the ringo-flow binary spawned per run
        #[arg(long, env = "RINGO_FLOW_SERVE_BINARY", value_hint = ValueHint::FilePath)]
        binary: Option<PathBuf>,
        /// Log level: trace/debug/info/warn/error (or a RUST_LOG-style directive)
        #[arg(long, env = "RINGO_FLOW_SERVE_LOG_LEVEL", default_value = "info")]
        log_level: String,
        /// Log format: `text` (human) or `json`
        #[arg(long, env = "RINGO_FLOW_SERVE_LOG_FORMAT", default_value = "text")]
        log_format: String,
        // The /metrics bearer token is read only from RINGO_FLOW_SERVE_METRICS_TOKEN
        // (a secret — kept out of the CLI args / process list).
    },
}

/// Parse repeated `--set key=value` flags into a map.
fn parse_overrides(set: &[String]) -> Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    for s in set {
        let (k, v) = s
            .split_once('=')
            .with_context(|| format!("--set expects key=value, got `{s}`"))?;
        if k.is_empty() {
            bail!("--set key must not be empty in `{s}`");
        }
        out.insert(k.to_string(), v.to_string());
    }
    Ok(out)
}

fn main() -> Result<()> {
    // Shell completion: when invoked by the completion script (COMPLETE env set),
    // emit candidates and exit; otherwise fall through to normal parsing.
    CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();
    match cli.command {
        Commands::Run {
            paths,
            set,
            env_file,
            scenario,
            tag,
            exclude_tag,
            log,
            sip_trace,
            save_audio,
            json,
            metrics,
            verbose,
            quiet,
            no_color,
            insecure_http,
        } => {
            // Backend log destination (process-global): off unless --log is given.
            match &log {
                None => {}
                Some(None) => ringo_core::log::init_stderr(),
                Some(Some(path)) => ringo_core::log::init_file(path),
            }
            // SIP trace — its own sink, independent of --log.
            match &sip_trace {
                None => {}
                Some(None) => ringo_core::sip_trace_stderr(),
                Some(Some(path)) => ringo_core::sip_trace_file(path),
            }
            // Color off if `--no-color`/`--no-ansi` or the `NO_COLOR` env var is set
            // (https://no-color.org); the reporter additionally requires a TTY.
            let color = !no_color && std::env::var_os("NO_COLOR").is_none();
            runtime::report::set_ansi_enabled(color);
            let overrides = parse_overrides(&set)?;
            // `--insecure-http` or the env var disables TLS verification for `http(...)`.
            let insecure_http =
                insecure_http || std::env::var_os("RINGO_FLOW_INSECURE_HTTP").is_some();
            let output = runtime::Output {
                json,
                quiet,
                verbose: verbose > 0,
                save_audio,
                insecure_http,
                metrics,
            };
            let filters = engine::Filters {
                name: scenario,
                tags: tag,
                exclude_tags: exclude_tag,
            };
            let result = script::run(&paths, output, overrides, filters, &env_file);
            ringo_core::shutdown();
            result
        }
        Commands::Check { file } => script::check(&file),
        Commands::Definitions { out } => script::write_definitions(&out),
        Commands::Docs { out } => script::write_book_api(&out),
        #[cfg(feature = "server")]
        Commands::Serve {
            config,
            listen,
            port,
            timeout,
            scheduler,
            metrics,
            binary,
            log_level,
            log_format,
        } => {
            serve::init_logging(&log_level, &log_format)?;
            let overrides = serve::Overrides {
                listen,
                port,
                timeout,
                scheduler,
                metrics_enabled: metrics,
                // Secret: env only, never a CLI flag.
                metrics_token: std::env::var("RINGO_FLOW_SERVE_METRICS_TOKEN")
                    .ok()
                    .filter(|s| !s.is_empty()),
                binary,
            };
            // The monitor is async (HTTP + schedulers); the other subcommands are
            // sync, so build a runtime just for this one.
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(serve::serve(&config, overrides))
        }
    }
}
