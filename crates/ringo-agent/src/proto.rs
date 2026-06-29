//! The line-delimited JSON (NDJSON) wire protocol between the flow parent and a
//! `ringo-flow agent` worker process.
//!
//! Three message kinds, one JSON object per line:
//!   * **parent → child** (worker stdin): [`ToWorker`] — a fire-and-forget
//!     [`Command`] or an id-correlated [`Query`] (wrapped in [`QueryEnvelope`]).
//!   * **child → parent** (worker stdout): [`FromWorker`] — an async [`WireEvent`]
//!     (the agent's [`AppEvent`] stream), an id-correlated [`Reply`] to a query,
//!     or a [`Headers`] push (received INVITE headers).
//!
//! The worker's **stderr** carries logs / SIP traces and is never part of the
//! protocol, so stdout stays clean for framing.

use ringo_core::account::{Account, BackendOptions};
use ringo_core::event::{AppEvent, InviteHeaders, MediaStats};
use serde::{Deserialize, Serialize};

// ── handshake ─────────────────────────────────────────────────────────────--

/// The full agent configuration the parent sends as the worker's first stdin
/// line (before any command/query). Carrying it on stdin rather than argv keeps
/// credentials out of the process table. The `Account`/`BackendOptions` are
/// embedded verbatim (they are `Serialize`/`Deserialize` in ringo-core), so the
/// worker rebuilds the *exact* account the in-process path would — adding a
/// field to either struct can't silently drift out of sync.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Label for the agent (used in logs / per-agent log file names).
    pub name: String,
    /// The SIP account to register.
    pub account: Account,
    /// Backend options (audio driver, timeouts, recording, …).
    pub options: BackendOptions,
}

// ── parent → child ──────────────────────────────────────────────────────────

/// A fire-and-forget phone action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    Register {
        aor: String,
        regint: u32,
    },
    Dial {
        number: String,
    },
    Hangup,
    HangupAll,
    Accept,
    Hold,
    Resume,
    Mute,
    Dtmf {
        digit: char,
    },
    SwitchLine {
        line: usize,
    },
    Transfer {
        uri: String,
    },
    AttendedTransferStart {
        uri: String,
    },
    AttendedTransferExec,
    AttendedTransferAbort,
    AddHeader {
        key: String,
        value: String,
    },
    RmHeader {
        key: String,
    },
    SetAudioSource {
        spec: String,
    },
    ArmInviteResponse {
        scode: u16,
        reason: String,
        headers: Vec<String>,
    },
    DisarmInviteResponse,
    /// Graceful shutdown request (also triggered by stdin EOF).
    Shutdown,
}

/// A request that expects exactly one [`Reply`] with the matching `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "query", rename_all = "snake_case")]
pub enum Query {
    /// RTP media stats for the active/last call.
    MediaStats,
    /// DTMF digits received so far.
    ReceivedDtmf,
    /// Goertzel tone analysis of the last `window_ms` of received audio.
    AnalyzeTone { freq: u32, window_ms: u64 },
    /// Write the agent's captured sent/received audio as WAV files named
    /// `<prefix>-sent.wav` / `<prefix>-recv.wav`; returns the paths written.
    SaveAudio { prefix: String },
    /// Number of active calls (for teardown's BYE-flush wait).
    CallCount,
}

/// A [`Query`] tagged with a correlation id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEnvelope {
    pub id: u64,
    #[serde(flatten)]
    pub query: Query,
}

/// Anything the parent writes to the worker's stdin. Untagged — disjoint by the
/// distinguishing key `cmd` (Command) vs `query` (QueryEnvelope). Do NOT add a
/// variant whose payload reuses either key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToWorker {
    Cmd(Command),
    Query(QueryEnvelope),
}

// ── child → parent ──────────────────────────────────────────────────────────

/// RTP media stats, mirroring [`MediaStats`] (which carries no serde derives).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WireMediaStats {
    pub rtt_ms: f64,
    pub jitter_ms: f64,
    pub rx_lost: i32,
    pub packet_loss_pct: f64,
    pub mos: f64,
}

impl From<MediaStats> for WireMediaStats {
    fn from(s: MediaStats) -> Self {
        Self {
            rtt_ms: s.rtt_ms,
            jitter_ms: s.jitter_ms,
            rx_lost: s.rx_lost,
            packet_loss_pct: s.packet_loss_pct,
            mos: s.mos,
        }
    }
}

impl From<WireMediaStats> for MediaStats {
    fn from(s: WireMediaStats) -> Self {
        Self {
            rtt_ms: s.rtt_ms,
            jitter_ms: s.jitter_ms,
            rx_lost: s.rx_lost,
            packet_loss_pct: s.packet_loss_pct,
            mos: s.mos,
        }
    }
}

/// Goertzel tone analysis result (mirrors `runtime::audio::ToneAnalysis`).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct WireToneAnalysis {
    pub score: f64,
    pub rms: f64,
    pub samples: usize,
}

/// The payload of a [`Reply`], one variant per [`Query`]. Externally tagged
/// (e.g. `{"call_count":2}`) so newtype variants over primitives/sequences
/// round-trip — internal tagging can't carry those.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyResult {
    MediaStats(Option<WireMediaStats>),
    Dtmf(String),
    Tone(WireToneAnalysis),
    Saved(Vec<String>),
    CallCount(u32),
}

/// A response to a [`Query`], correlated by `id` (e.g.
/// `{"reply":7,"result":{"call_count":2}}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub reply: u64,
    pub result: ReplyResult,
}

/// A push of received INVITE headers (keyed by SIP Call-ID).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Headers {
    pub headers: InviteHeaders,
}

/// Readiness handshake: the worker emits this as its FIRST stdout line once the
/// backend is up, so [`crate::client::ProcessClient::spawn`] can surface a bad
/// config / failed backend at spawn time instead of as a later query timeout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ready {
    pub ready: bool,
}

/// Anything the worker writes to its stdout. Untagged — the variants are
/// disjoint by their distinguishing key: `ready` / `event` / `reply` / `headers`.
/// Do NOT add a variant whose payload reuses one of those keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FromWorker {
    Ready(Ready),
    Event(WireEvent),
    Reply(Reply),
    Headers(Headers),
}

/// A serializable mirror of [`AppEvent`] (which lives in `ringo-core` and
/// carries no serde derives).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WireEvent {
    Registering {
        account: String,
    },
    RegisterOk {
        account: String,
    },
    RegisterFailed {
        reason: String,
    },
    Unregistered {
        account: String,
    },
    CallIncoming {
        call_id: String,
        number: String,
        display_name: Option<String>,
    },
    CallOutgoing {
        call_id: String,
        number: String,
    },
    CallRinging {
        call_id: String,
    },
    CallEstablished {
        call_id: String,
    },
    CallClosed {
        call_id: String,
        reason: String,
        error: bool,
    },
    VoicemailStatus {
        waiting: bool,
        new_count: u32,
    },
    Response {
        ok: bool,
        data: String,
    },
    Unknown {
        class: String,
        type_: String,
    },
    BackendConnectFailed {
        reason: String,
    },
}

impl From<&AppEvent> for WireEvent {
    fn from(e: &AppEvent) -> Self {
        match e {
            AppEvent::Registering { account } => Self::Registering {
                account: account.clone(),
            },
            AppEvent::RegisterOk { account } => Self::RegisterOk {
                account: account.clone(),
            },
            AppEvent::RegisterFailed { reason } => Self::RegisterFailed {
                reason: reason.clone(),
            },
            AppEvent::Unregistered { account } => Self::Unregistered {
                account: account.clone(),
            },
            AppEvent::CallIncoming {
                call_id,
                number,
                display_name,
            } => Self::CallIncoming {
                call_id: call_id.clone(),
                number: number.clone(),
                display_name: display_name.clone(),
            },
            AppEvent::CallOutgoing { call_id, number } => Self::CallOutgoing {
                call_id: call_id.clone(),
                number: number.clone(),
            },
            AppEvent::CallRinging { call_id } => Self::CallRinging {
                call_id: call_id.clone(),
            },
            AppEvent::CallEstablished { call_id } => Self::CallEstablished {
                call_id: call_id.clone(),
            },
            AppEvent::CallClosed {
                call_id,
                reason,
                error,
            } => Self::CallClosed {
                call_id: call_id.clone(),
                reason: reason.clone(),
                error: *error,
            },
            AppEvent::VoicemailStatus { waiting, new_count } => Self::VoicemailStatus {
                waiting: *waiting,
                new_count: *new_count,
            },
            AppEvent::Response { ok, data } => Self::Response {
                ok: *ok,
                data: data.clone(),
            },
            AppEvent::Unknown { class, type_ } => Self::Unknown {
                class: class.clone(),
                type_: type_.clone(),
            },
            AppEvent::BackendConnectFailed { reason } => Self::BackendConnectFailed {
                reason: reason.clone(),
            },
        }
    }
}

impl From<WireEvent> for AppEvent {
    fn from(w: WireEvent) -> Self {
        match w {
            WireEvent::Registering { account } => AppEvent::Registering { account },
            WireEvent::RegisterOk { account } => AppEvent::RegisterOk { account },
            WireEvent::RegisterFailed { reason } => AppEvent::RegisterFailed { reason },
            WireEvent::Unregistered { account } => AppEvent::Unregistered { account },
            WireEvent::CallIncoming {
                call_id,
                number,
                display_name,
            } => AppEvent::CallIncoming {
                call_id,
                number,
                display_name,
            },
            WireEvent::CallOutgoing { call_id, number } => {
                AppEvent::CallOutgoing { call_id, number }
            }
            WireEvent::CallRinging { call_id } => AppEvent::CallRinging { call_id },
            WireEvent::CallEstablished { call_id } => AppEvent::CallEstablished { call_id },
            WireEvent::CallClosed {
                call_id,
                reason,
                error,
            } => AppEvent::CallClosed {
                call_id,
                reason,
                error,
            },
            WireEvent::VoicemailStatus { waiting, new_count } => {
                AppEvent::VoicemailStatus { waiting, new_count }
            }
            WireEvent::Response { ok, data } => AppEvent::Response { ok, data },
            WireEvent::Unknown { class, type_ } => AppEvent::Unknown { class, type_ },
            WireEvent::BackendConnectFailed { reason } => AppEvent::BackendConnectFailed { reason },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_roundtrips_as_to_worker() {
        let line = serde_json::to_string(&ToWorker::Cmd(Command::Dial {
            number: "sip:1@example.com".into(),
        }))
        .unwrap();
        assert_eq!(line, r#"{"cmd":"dial","number":"sip:1@example.com"}"#);
        assert!(matches!(
            serde_json::from_str::<ToWorker>(&line).unwrap(),
            ToWorker::Cmd(Command::Dial { .. })
        ));
    }

    #[test]
    fn query_envelope_roundtrips_as_to_worker() {
        let line = serde_json::to_string(&ToWorker::Query(QueryEnvelope {
            id: 7,
            query: Query::AnalyzeTone {
                freq: 1000,
                window_ms: 2000,
            },
        }))
        .unwrap();
        let back: ToWorker = serde_json::from_str(&line).unwrap();
        match back {
            ToWorker::Query(QueryEnvelope {
                id,
                query: Query::AnalyzeTone { freq, window_ms },
            }) => {
                assert_eq!((id, freq, window_ms), (7, 1000, 2000));
            }
            _ => panic!("expected analyze_tone query"),
        }
    }

    #[test]
    fn event_and_reply_disambiguate_as_from_worker() {
        let ev = serde_json::to_string(&FromWorker::Event(WireEvent::RegisterOk {
            account: "a".into(),
        }))
        .unwrap();
        assert!(matches!(
            serde_json::from_str::<FromWorker>(&ev).unwrap(),
            FromWorker::Event(WireEvent::RegisterOk { .. })
        ));

        let rep = serde_json::to_string(&FromWorker::Reply(Reply {
            reply: 7,
            result: ReplyResult::CallCount(2),
        }))
        .unwrap();
        match serde_json::from_str::<FromWorker>(&rep).unwrap() {
            FromWorker::Reply(Reply {
                reply,
                result: ReplyResult::CallCount(n),
            }) => {
                assert_eq!((reply, n), (7, 2));
            }
            _ => panic!("expected call_count reply"),
        }
    }

    #[test]
    fn app_event_mirrors_both_ways() {
        let ev = AppEvent::CallIncoming {
            call_id: "abc".into(),
            number: "sip:1@x".into(),
            display_name: Some("Bob".into()),
        };
        let line = serde_json::to_string(&WireEvent::from(&ev)).unwrap();
        let back: AppEvent = serde_json::from_str::<WireEvent>(&line).unwrap().into();
        assert!(matches!(back, AppEvent::CallIncoming { .. }));
    }

    #[test]
    fn ready_disambiguates_from_an_event() {
        let ready = serde_json::to_string(&FromWorker::Ready(Ready { ready: true })).unwrap();
        assert!(matches!(
            serde_json::from_str::<FromWorker>(&ready).unwrap(),
            FromWorker::Ready(Ready { ready: true })
        ));
        // An event must NOT be mis-parsed as Ready (disjoint keys).
        let ev = serde_json::to_string(&FromWorker::Event(WireEvent::RegisterOk {
            account: "a".into(),
        }))
        .unwrap();
        assert!(matches!(
            serde_json::from_str::<FromWorker>(&ev).unwrap(),
            FromWorker::Event(_)
        ));
    }

    #[test]
    fn agent_config_roundtrips_with_embedded_account() {
        let mut cfg = AgentConfig {
            name: "A".into(),
            ..Default::default()
        };
        cfg.account.username = "alice".into();
        cfg.account.catchall = true;
        cfg.options.audio_driver = Some("aubridge".into());
        let line = serde_json::to_string(&cfg).unwrap();
        let back: AgentConfig = serde_json::from_str(&line).unwrap();
        assert_eq!(back.name, "A");
        assert_eq!(back.account.username, "alice");
        assert!(back.account.catchall);
        assert_eq!(back.options.audio_driver.as_deref(), Some("aubridge"));
    }
}
