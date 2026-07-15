//! The wire protocol between the parent and a `ringo-flow agent` worker process.
//!
//! Length-prefixed frames (`[kind][len][payload]`, see [`write_frame`]) so JSON
//! control and raw binary audio share one stream per direction:
//!   * **parent → child** (worker stdin): control frames carrying [`ToWorker`]
//!     (a [`Command`] or an id-correlated [`Query`]), plus raw audio frames (TTS
//!     PCM played into the call after `StartTxAudio`).
//!   * **child → parent** (worker stdout): control frames carrying [`FromWorker`]
//!     (events, query replies, header pushes, readiness), plus raw audio frames
//!     (received-call PCM after `StartRxAudio`, preceded by `RxAudioStarted`).
//!
//! The worker's **stderr** carries logs / SIP traces and is never part of the
//! protocol, so the framed stream stays clean.

use ringo_core::account::{Account, BackendOptions};
use ringo_core::event::{AppEvent, InviteHeaders, MediaStats};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

// ── wire framing ──────────────────────────────────────────────────────────--
//
// The wire is length-prefixed frames (not line-delimited), so raw binary audio
// and JSON control can share one stream without escaping: `[kind:u8][len:u32
// LE][payload]`. A control frame's payload is JSON (a `ToWorker`/`FromWorker`);
// an audio frame's payload is raw mono s16le PCM.

/// Frame kind: JSON control message.
pub(crate) const FRAME_CONTROL: u8 = 0;
/// Frame kind: raw mono s16le PCM audio.
pub(crate) const FRAME_AUDIO: u8 = 1;

/// Reject absurd frame lengths from a corrupt stream (far above any real frame).
const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;

/// Write one frame: `[kind][len][payload]`, flushed.
pub(crate) fn write_frame<W: Write>(w: &mut W, kind: u8, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame too large",
        ));
    }
    w.write_all(&[kind])?;
    w.write_all(&(payload.len() as u32).to_le_bytes())?;
    w.write_all(payload)?;
    w.flush()
}

/// Read one frame; `Ok(None)` only on a *clean* EOF at a frame boundary (no
/// bytes of a new frame seen). EOF partway through the header or payload is a
/// torn frame and surfaces as an error, not a clean close.
pub(crate) fn read_frame<R: Read>(r: &mut R) -> io::Result<Option<(u8, Vec<u8>)>> {
    let mut hdr = [0u8; 5];
    // Read the header byte-by-byte so we can tell "nothing arrived" (clean EOF)
    // from "EOF mid-header" (torn frame). read_exact can't distinguish them.
    let mut filled = 0;
    while filled < hdr.len() {
        match r.read(&mut hdr[filled..]) {
            Ok(0) if filled == 0 => return Ok(None), // clean frame-boundary EOF
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "EOF in frame header",
                ));
            }
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    let kind = hdr[0];
    let len = u32::from_le_bytes([hdr[1], hdr[2], hdr[3], hdr[4]]) as usize;
    if len > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok(Some((kind, payload)))
}

/// Mono s16 samples → little-endian audio-frame bytes.
pub(crate) fn pcm_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut v = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

/// Little-endian audio-frame bytes → mono s16 samples (trailing odd byte ignored).
pub(crate) fn bytes_to_pcm(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

// ── handshake ─────────────────────────────────────────────────────────────--

/// Wire-protocol version. Parent and worker are normally the same binary, but a
/// stale worker (old binary on `$PATH`, a half-finished upgrade) would otherwise
/// desync silently on the framed stream. The worker rejects a mismatching
/// version at the handshake instead. Bump on any incompatible wire change.
pub(crate) const PROTO_VERSION: u32 = 1;

/// The first frame the parent sends: the protocol version plus the agent config.
/// Versioning lives here, on the wire envelope, so the public [`AgentConfig`]
/// stays a plain user-facing struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Handshake {
    /// Sender's [`PROTO_VERSION`]; the worker bails if it doesn't match its own.
    pub proto_version: u32,
    /// The agent configuration to spawn.
    pub config: AgentConfig,
}

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
    /// Switch the agent's audio source to live-streamed PCM at `rate` Hz; raw
    /// audio frames (kind=1) on stdin are then played into the call (TTS).
    StartTxAudio {
        rate: u32,
    },
    /// Begin streaming the agent's received audio: the worker emits an
    /// `RxAudioStarted` control message then raw audio frames (kind=1) on stdout.
    StartRxAudio,
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

/// The payload of a [`Reply`], one variant per [`Query`]. Externally tagged
/// (e.g. `{"call_count":2}`) so newtype variants over primitives/sequences
/// round-trip — internal tagging can't carry those.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyResult {
    MediaStats(Option<WireMediaStats>),
    Dtmf(String),
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

/// Readiness handshake: the worker emits this as its FIRST stdout message once
/// the backend is up, so [`crate::client::ProcessClient::spawn`] can surface a
/// bad config / failed backend at spawn time instead of as a later query timeout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ready {
    pub ready: bool,
}

/// Sent once after `StartRxAudio`, before the raw RX audio frames, so the parent
/// knows the sample rate of the mono s16 frames that follow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RxAudioStarted {
    pub rx_audio_rate: u32,
}

/// A control message the worker writes to its stdout (the `control`-kind frame
/// payload). Untagged — the variants are disjoint by their distinguishing key:
/// `ready` / `event` / `reply` / `headers` / `rx_audio_rate`. Do NOT add a
/// variant whose payload reuses one of those keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FromWorker {
    Ready(Ready),
    Event(WireEvent),
    Reply(Reply),
    Headers(Headers),
    RxAudioStarted(RxAudioStarted),
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
    CallDeflected {
        from: String,
        display_name: Option<String>,
        target: String,
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
            AppEvent::CallDeflected {
                from,
                display_name,
                target,
            } => Self::CallDeflected {
                from: from.clone(),
                display_name: display_name.clone(),
                target: target.clone(),
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
            WireEvent::CallDeflected {
                from,
                display_name,
                target,
            } => AppEvent::CallDeflected {
                from,
                display_name,
                target,
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
            query: Query::SaveAudio {
                prefix: "rec".into(),
            },
        }))
        .unwrap();
        let back: ToWorker = serde_json::from_str(&line).unwrap();
        match back {
            ToWorker::Query(QueryEnvelope {
                id,
                query: Query::SaveAudio { prefix },
            }) => {
                assert_eq!((id, prefix.as_str()), (7, "rec"));
            }
            _ => panic!("expected save_audio query"),
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
    fn handshake_carries_version_and_config() {
        let mut cfg = AgentConfig {
            name: "A".into(),
            ..Default::default()
        };
        cfg.account.username = "alice".into();
        let hs = Handshake {
            proto_version: PROTO_VERSION,
            config: cfg,
        };
        let line = serde_json::to_string(&hs).unwrap();
        let back: Handshake = serde_json::from_str(&line).unwrap();
        assert_eq!(back.proto_version, PROTO_VERSION);
        assert_eq!(back.config.account.username, "alice");
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

    #[test]
    fn frames_roundtrip_control_then_audio() {
        let mut buf: Vec<u8> = Vec::new();
        write_frame(&mut buf, FRAME_CONTROL, b"{\"cmd\":\"accept\"}").unwrap();
        write_frame(&mut buf, FRAME_AUDIO, &pcm_to_bytes(&[1, -2, 3])).unwrap();
        let mut r = &buf[..];
        let (k1, p1) = read_frame(&mut r).unwrap().unwrap();
        assert_eq!(k1, FRAME_CONTROL);
        assert_eq!(p1, b"{\"cmd\":\"accept\"}");
        let (k2, p2) = read_frame(&mut r).unwrap().unwrap();
        assert_eq!(k2, FRAME_AUDIO);
        assert_eq!(bytes_to_pcm(&p2), vec![1, -2, 3]);
        assert!(read_frame(&mut r).unwrap().is_none()); // clean EOF
    }

    #[test]
    fn audio_frame_can_contain_newline_bytes() {
        // 0x0A (\n) in PCM must NOT break framing (the whole point of length-prefix).
        let samples = vec![0x0A0Ai16, 0x000A, -1];
        let mut buf = Vec::new();
        write_frame(&mut buf, FRAME_AUDIO, &pcm_to_bytes(&samples)).unwrap();
        let (_k, p) = read_frame(&mut &buf[..]).unwrap().unwrap();
        assert_eq!(bytes_to_pcm(&p), samples);
    }

    #[test]
    fn rx_audio_started_disambiguates_from_event_and_reply() {
        let started = serde_json::to_string(&FromWorker::RxAudioStarted(RxAudioStarted {
            rx_audio_rate: 8000,
        }))
        .unwrap();
        assert!(matches!(
            serde_json::from_str::<FromWorker>(&started).unwrap(),
            FromWorker::RxAudioStarted(RxAudioStarted {
                rx_audio_rate: 8000
            })
        ));
        // Neither an event nor a reply carries the `rx_audio_rate` key.
        let ev = serde_json::to_string(&FromWorker::Event(WireEvent::RegisterOk {
            account: "a".into(),
        }))
        .unwrap();
        let rep = serde_json::to_string(&FromWorker::Reply(Reply {
            reply: 1,
            result: ReplyResult::CallCount(0),
        }))
        .unwrap();
        assert!(matches!(
            serde_json::from_str::<FromWorker>(&ev).unwrap(),
            FromWorker::Event(_)
        ));
        assert!(matches!(
            serde_json::from_str::<FromWorker>(&rep).unwrap(),
            FromWorker::Reply(_)
        ));
    }

    #[test]
    fn read_frame_distinguishes_clean_eof_from_torn_header() {
        // Nothing buffered → clean frame-boundary EOF.
        assert!(read_frame(&mut &b""[..]).unwrap().is_none());
        // A partial header (2 of 5 bytes) is a torn frame, not a clean close.
        let err = read_frame(&mut &[FRAME_CONTROL, 0x01][..]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn read_frame_rejects_oversized_length() {
        // Header claiming a payload above MAX_FRAME_LEN must be rejected, not allocated.
        let mut buf = vec![FRAME_AUDIO];
        buf.extend_from_slice(&(MAX_FRAME_LEN as u32 + 1).to_le_bytes());
        let err = read_frame(&mut &buf[..]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn read_frame_errors_on_truncated_payload() {
        // Full header but a short payload (EOF mid-payload) is a torn frame.
        let mut buf = vec![FRAME_AUDIO];
        buf.extend_from_slice(&4u32.to_le_bytes()); // claims 4 bytes
        buf.extend_from_slice(&[1, 2]); // only 2 follow
        let err = read_frame(&mut &buf[..]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn audio_commands_disambiguate_as_to_worker_cmd() {
        // The audio control commands must parse as Cmd, not Query (untagged split).
        for cmd in [Command::StartRxAudio, Command::StartTxAudio { rate: 8000 }] {
            let line = serde_json::to_string(&ToWorker::Cmd(cmd)).unwrap();
            assert!(
                matches!(
                    serde_json::from_str::<ToWorker>(&line).unwrap(),
                    ToWorker::Cmd(_)
                ),
                "expected Cmd for {line}"
            );
        }
    }
}
