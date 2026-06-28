//! Runtime engine shared by scenarios: one baresip [`session::AgentSession`] per
//! agent, event-reduced [`state::AgentState`], audio analysis and reporting. The
//! Rhai host in [`crate::script`] drives these; this module has no scenario
//! language of its own.

pub(crate) mod audio;
pub(crate) mod report;
pub(crate) mod session;
pub(crate) mod state;
pub(crate) mod wait;

pub(crate) use wait::wait_holding;

use ringo_core::account::BackendOptions;

/// The backend options every ringo-flow agent runs with.
///
/// `aubridge` selects headless mode (no sound hardware, so calls establish in CI
/// — unlike auto-detected pipewire/pulse/alsa, which fail to open a device and
/// abort baresip). In that mode the backend routes both the audio source and
/// player through ringo's own in-process module (see `ringo_core` ausrc):
/// `send-audio` sets a per-agent tone/file/silence the source renders (surviving
/// re-INVITEs — transfer/hold/line switch), and the player captures received
/// audio for `verify-audio`/`--save-audio`. A silent call stays silent until
/// `send-audio` is called.
pub(crate) fn agent_options() -> BackendOptions {
    BackendOptions {
        audio_driver: Some("aubridge".into()),
        user_agent: Some(concat!("ringo-flow/", env!("CARGO_PKG_VERSION")).into()),
        // A scenario drives hold/resume explicitly, so disable baresip's
        // auto-hold-other-calls (which would silently hold a call when another
        // arrives and skew assertions).
        hold_other_calls: Some(false),
        // record_audio (full in-process capture) is set per run from --save-audio
        // by the caller; verification uses the rolling window regardless.
        ..Default::default()
    }
}

/// How a run reports progress (set from the CLI).
#[derive(Debug, Clone, Copy, Default)]
pub struct Output {
    /// NDJSON instead of the human log.
    pub json: bool,
    /// Only failures + the final result.
    pub quiet: bool,
    /// Add observed state to every assertion.
    pub verbose: bool,
    /// Copy each agent's call recordings (sent/received WAV) to the cwd.
    pub save_audio: bool,
    /// Disable TLS certificate verification for `http(...)` (DANGER).
    pub insecure_http: bool,
    /// Emit per-agent media-quality metrics at each scenario's end (`--metrics`).
    pub metrics: bool,
}
