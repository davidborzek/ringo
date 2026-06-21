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

use ringo_core::baresip::BaresipOptions;

/// The baresip options every ringo-flow agent runs with.
///
/// SIP trace on: lets sessions recover inbound INVITE headers (for `header`
/// assertions) that the ctrl_tcp events don't expose. `aubridge` is a virtual
/// loopback audio device: scenarios need no real sound hardware, so calls
/// establish in CI/headless (unlike the auto-detected pipewire/pulse/alsa, which
/// fail to open a device and abort baresip). Player and source share the device
/// name, so aubridge couples them: the recv path is clocked (sndfile can record
/// dump-…-dec.wav) and, until a `send-audio` overrides the source, nothing is
/// injected, so a silent call stays silent — the loopback only echoes audio that
/// is actually sent. `send-audio` switches the source via `ausrc ausine,<freq>` /
/// `ausrc aufile,<path>`; those drivers only exist if their modules are loaded,
/// hence the `extra` module lines.
pub(crate) fn agent_options() -> BaresipOptions {
    BaresipOptions {
        sip_trace: true,
        audio_driver: Some("aubridge".into()),
        record_audio: true,
        extra: vec![
            ("module".into(), "ausine.so".into()),
            ("module".into(), "aufile.so".into()),
        ],
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
    /// Dump each agent's baresip log (to stderr) at the end.
    pub logs: bool,
    /// Copy each agent's call recordings (sent/received WAV) to the cwd.
    pub save_audio: bool,
    /// Disable TLS certificate verification for `http(...)` (DANGER).
    pub insecure_http: bool,
}
