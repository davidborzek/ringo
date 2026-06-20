mod app;
mod baresip;
mod client;
mod config;
mod contacts;
mod control;
mod event;
mod form;
mod header;
mod history;
mod hooks;
mod log;
mod notify;
mod phone;
mod picker;
mod profile;
mod tui;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{
    engine::{ArgValueCandidates, CompletionCandidate},
    env::CompleteEnv,
};

fn profile_candidates() -> Vec<CompletionCandidate> {
    profile::list_names()
        .unwrap_or_default()
        .into_iter()
        .map(CompletionCandidate::new)
        .collect()
}

/// Completion candidates for `-t`: both profile names and PIDs of running
/// sessions, so awkward profile names can be targeted by PID instead.
fn target_candidates() -> Vec<CompletionCandidate> {
    control::list_running()
        .into_iter()
        .flat_map(|s| {
            [
                CompletionCandidate::new(s.pid.to_string()),
                CompletionCandidate::new(s.profile),
            ]
        })
        .collect()
}

/// Completion candidates for the `control` command name.
fn control_command_candidates() -> Vec<CompletionCandidate> {
    [
        "dial", "hangup", "accept", "hold", "resume", "mute", "dtmf", "transfer", "status", "list",
    ]
    .iter()
    .map(|c| CompletionCandidate::new(*c))
    .collect()
}

#[derive(Parser)]
#[command(
    name = "ringo",
    version,
    about = "A TUI softphone for managing and launching baresip SIP accounts",
    long_about = "ringo wraps baresip with a terminal UI for managing multiple SIP profiles.\n\
                  Run without arguments to open the interactive profile picker."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start baresip with a profile (opens picker if no name given) [default]
    Start {
        /// Profile name — skips the picker
        #[arg(add = ArgValueCandidates::new(profile_candidates))]
        profile: Option<String>,
        /// Disable desktop notifications
        #[arg(long)]
        no_notify: bool,
    },

    /// List all profiles
    List {
        /// Print only profile names, one per line (for scripting)
        #[arg(short, long)]
        plain: bool,

        /// Custom output format with placeholders: {name}, {username}, {domain},
        /// {display_name}, {transport}, {aor}, {auth_user}, {outbound},
        /// {stun_server}, {media_enc} (implies --plain)
        #[arg(short, long)]
        format: Option<String>,

        /// Output as JSON array
        #[arg(short, long)]
        json: bool,
    },

    /// Control a running ringo session
    ///
    /// Examples:
    ///   ringo control -t work dial 4711
    ///   ringo control -t 215709 hangup
    ///   ringo control list
    #[command(alias = "ctl")]
    Control {
        /// Target session: a profile name, or a PID for awkward names /
        /// multiple instances (see `ringo control list`)
        #[arg(short = 't', long, add = ArgValueCandidates::new(target_candidates))]
        target: Option<String>,

        /// Command: dial, hangup, accept, hold, resume, mute, dtmf, transfer, status, list
        #[arg(add = ArgValueCandidates::new(control_command_candidates))]
        command: String,

        /// Command arguments (e.g. the number for `dial`, URI for `transfer`)
        args: Vec<String>,

        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Start {
        profile: None,
        no_notify: false,
    }) {
        Commands::Start { profile, no_notify } => app::run(profile, !no_notify)?,
        Commands::List {
            plain,
            format,
            json,
        } => profile::list(plain, format, json)?,
        Commands::Control {
            target,
            command,
            args,
            json,
        } => run_control(target, command, args, json)?,
    }

    Ok(())
}

fn run_control(
    target: Option<String>,
    command: String,
    args: Vec<String>,
    json: bool,
) -> Result<()> {
    // `list` enumerates sessions and needs no target.
    if command == "list" {
        let sessions = control::list_running();
        if json {
            println!("{}", serde_json::to_string_pretty(&sessions)?);
        } else if sessions.is_empty() {
            println!("No running ringo sessions.");
        } else {
            for s in &sessions {
                println!("{}\t{}\t{}", s.pid, s.profile, s.aor);
            }
        }
        return Ok(());
    }

    let target = match target {
        Some(t) => t,
        None => {
            return control_error(
                json,
                "Missing target. Use `-t <profile|pid>` (see `ringo control list`).",
            );
        }
    };
    let info = match resolve_session(&target) {
        Ok(i) => i,
        Err(e) => return control_error(json, &e.to_string()),
    };

    let resp = control::send(&info.socket_path, &command, &args.join(" "))?;

    if json {
        // `status` returns a structured object in `data`; everything else a
        // plain message string. Embed accordingly under a uniform envelope.
        let data = if command == "status" {
            serde_json::from_str(&resp.data).unwrap_or_else(|_| serde_json::json!(resp.data))
        } else {
            serde_json::json!(resp.data)
        };
        let envelope = serde_json::json!({
            "ok": resp.ok,
            "data": data,
            "error": resp.error,
        });
        println!("{}", serde_json::to_string_pretty(&envelope)?);
        if !resp.ok {
            std::process::exit(1);
        }
        return Ok(());
    }

    if !resp.ok {
        return Err(anyhow::anyhow!(
            resp.error.unwrap_or_else(|| "command failed".into())
        ));
    }
    if command == "status" {
        print_status_text(&resp.data);
    } else if !resp.data.is_empty() {
        println!("{}", resp.data);
    }
    Ok(())
}

/// Report a client-side control error, as a JSON envelope on stdout (exit 1) in
/// JSON mode, or as a plain anyhow error otherwise.
fn control_error(json: bool, msg: &str) -> Result<()> {
    if json {
        let envelope = serde_json::json!({"ok": false, "data": null, "error": msg});
        println!("{}", serde_json::to_string_pretty(&envelope)?);
        std::process::exit(1);
    }
    Err(anyhow::anyhow!(msg.to_string()))
}

/// Render the structured `status` payload as human-readable text.
fn print_status_text(data: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
        println!("{data}");
        return;
    };
    let str_field = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("");
    let calls = v.get("calls").and_then(|c| c.as_array());
    println!("profile: {}", str_field("profile"));
    println!("account: {}", str_field("account"));
    println!("registration: {}", str_field("registration"));
    println!(
        "muted: {}",
        v.get("muted").and_then(|x| x.as_bool()).unwrap_or(false)
    );
    println!("calls: {}", calls.map(|c| c.len()).unwrap_or(0));
    for c in calls.into_iter().flatten() {
        let idx = c.get("index").and_then(|x| x.as_u64()).unwrap_or(0);
        let dir = c.get("direction").and_then(|x| x.as_str()).unwrap_or("");
        let peer = c.get("peer").and_then(|x| x.as_str()).unwrap_or("");
        let state = c.get("state").and_then(|x| x.as_str()).unwrap_or("");
        println!("  [{idx}] {dir} {peer} {state}");
    }
}

/// Resolve a target string to exactly one running session. A purely numeric
/// target is matched against PIDs first; otherwise it is treated as a profile
/// name. Profiles with multiple live instances must be targeted by PID.
fn resolve_session(target: &str) -> Result<control::SessionInfo> {
    let running = control::list_running();

    if let Ok(pid) = target.parse::<u32>() {
        if let Some(s) = running.iter().find(|s| s.pid == pid) {
            return Ok(s.clone());
        }
    }

    let mut matches: Vec<_> = running
        .into_iter()
        .filter(|s| s.profile == target)
        .collect();
    match matches.len() {
        0 => Err(anyhow::anyhow!(
            "No running session matching '{}'. Try `ringo control list`.",
            target
        )),
        1 => Ok(matches.remove(0)),
        _ => {
            let pids: Vec<String> = matches.iter().map(|s| s.pid.to_string()).collect();
            Err(anyhow::anyhow!(
                "Multiple sessions for profile '{}' (pids: {}). Target a PID instead: -t <PID>.",
                target,
                pids.join(", ")
            ))
        }
    }
}
