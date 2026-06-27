mod engine;
mod runtime;
mod script;

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
    }
}
