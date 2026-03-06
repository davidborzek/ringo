mod app;
mod baresip;
mod client;
mod config;
mod event;
mod form;
mod history;
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
        Commands::List { plain } => profile::list(plain)?,
    }

    Ok(())
}
