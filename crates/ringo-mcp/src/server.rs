//! The MCP tool surface: a thin, typed layer over the [`Hub`][crate::hub::Hub].
//!
//! Every tool takes an `agent` name (plus params where needed) and returns a
//! compact JSON payload. The first tool call for an agent spawns its worker
//! process (lazy, single-flight — see the hub docs). Call control is
//! fire-and-forget (the worker applies it); outcomes are observed either by
//! polling `agent_status` or by blocking on `wait_event`.

use crate::hub::Hub;
use anyhow::Result;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::schemars;
use rmcp::tool_handler;
use rmcp::tool_router;
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt, tool, transport::stdio};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// Tool groups for the `--disable-<group>` and `--enabled-tools` flags —
/// the mcp-grafana-style way to trim the surface (context tokens, read-only
/// observers, lockdown profiles). A flag may name a group OR an individual tool.
pub(crate) const TOOL_GROUPS: &[(&str, &[&str])] = &[
    ("discovery", &["list_agents", "agent_status"]),
    (
        "call-control",
        &[
            "dial",
            "accept",
            "hangup",
            "hangup_all",
            "hold",
            "resume",
            "mute",
            "send_dtmf",
            "transfer",
        ],
    ),
    ("audio", &["play"]),
    ("headers", &["call_headers", "add_header", "rm_header"]),
    ("events", &["wait_event"]),
    ("streams", &["stream_open", "stream_close"]),
    ("recording", &["save_audio"]),
    ("lifecycle", &["agent_stop"]),
];

/// Every tool name (union of all groups — keeps the table honest).
fn all_tool_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = TOOL_GROUPS
        .iter()
        .flat_map(|(_, tools)| tools.iter().copied())
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Expand one `--enabled-tools`/`--disable-` name (group or tool) to tool
/// names. `None` = unknown name.
fn expand_tool_name(name: &str) -> Option<Vec<&'static str>> {
    if let Some((_, tools)) = TOOL_GROUPS.iter().find(|(g, _)| *g == name) {
        return Some(tools.to_vec());
    }
    if let Some(t) = all_tool_names().into_iter().find(|t| *t == name) {
        return Some(vec![t]);
    }
    None
}

/// How many tools exist in total (the group table's union).
pub fn total_tool_count() -> usize {
    all_tool_names().len()
}

/// Resolve the flags to the list of tools to disable (passed to the router).
/// `enabled`: the `--enabled-tools` allowlist (None = all enabled);
/// `disable`: the `--disable-<name>` flag values, applied on top.
pub fn resolve_disabled_tools(
    enabled: Option<&[String]>,
    disable: &[String],
) -> Result<Vec<String>> {
    let all = all_tool_names();
    let allowed: Option<std::collections::HashSet<&str>> = match enabled {
        None => None,
        Some(names) => {
            let mut set = std::collections::HashSet::new();
            for n in names {
                let expanded = expand_tool_name(n).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown tool or group `{n}`; valid groups: {}; valid tools: {}",
                        TOOL_GROUPS
                            .iter()
                            .map(|(g, _)| *g)
                            .collect::<Vec<_>>()
                            .join(", "),
                        all.join(", ")
                    )
                })?;
                set.extend(expanded);
            }
            Some(set)
        }
    };
    let mut disabled: Vec<String> = Vec::new();
    for n in disable {
        let expanded = expand_tool_name(n).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown tool or group `{n}`; valid groups: {}; valid tools: {}",
                TOOL_GROUPS
                    .iter()
                    .map(|(g, _)| *g)
                    .collect::<Vec<_>>()
                    .join(", "),
                all.join(", ")
            )
        })?;
        disabled.extend(expanded.into_iter().map(String::from));
    }
    if let Some(allowed) = allowed {
        // Allowlist mode: everything not allowed is disabled.
        for t in all {
            if !allowed.contains(t) && !disabled.iter().any(|d| d == t) {
                disabled.push(t.to_string());
            }
        }
    }
    disabled.sort();
    disabled.dedup();
    Ok(disabled)
}

/// Cap on a single `wait_event` timeout, so a confused model can't park a
/// tool call for hours.
const MAX_WAIT_MS: u64 = 120_000;
/// Default `wait_event` timeout.
const DEFAULT_WAIT_MS: u64 = 30_000;

/// The MCP server state: the agent hub plus the generated tool router.
#[derive(Clone)]
pub struct TelephonyServer {
    hub: Arc<Hub>,
    /// The full tool set with the CLI-flag disables applied. Bound into the
    /// generated dispatch via `#[tool_handler(router = …)]` — the plain
    /// `#[tool_handler]` default would rebuild a fresh, unfiltered router.
    tool_router: ToolRouter<Self>,
}

impl TelephonyServer {
    fn new(hub: Arc<Hub>, disabled_tools: Vec<String>) -> Self {
        let mut tool_router = Self::build_tool_router();
        for name in disabled_tools {
            tool_router.disable_route(name);
        }
        Self { hub, tool_router }
    }

    /// Look up (spawning on first use) the named agent's running instance.
    /// Unknown agents are a client error; a failed spawn is an internal one
    /// (the config was validated at startup, so this is a runtime problem).
    async fn agent(&self, name: &str) -> Result<Arc<crate::hub::Agent>, McpError> {
        self.hub.get(name).await.map_err(|e| {
            if e.to_string().contains("unknown agent") {
                McpError::invalid_params(e.to_string(), None)
            } else {
                McpError::internal_error(e.to_string(), None)
            }
        })
    }
}

// Parameter structs ──────────────────────────────────────────────────────────

/// Identifies an agent by its config name.
#[derive(Deserialize, schemars::JsonSchema)]
struct AgentParam {
    /// Agent name from the config file.
    agent: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct DialParam {
    /// Agent name from the config file.
    agent: String,
    /// SIP URI, `user@host`, or bare number/extension (resolved to
    /// `sip:<target>@<agent domain>`).
    target: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct DtmfParam {
    /// Agent name from the config file.
    agent: String,
    /// One DTMF digit: `0-9`, `*`, `#`, or `A-F`.
    digit: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct TransferParam {
    /// Agent name from the config file.
    agent: String,
    /// SIP URI, `user@host`, or bare number/extension (resolved to
    /// `sip:<target>@<agent domain>`).
    target: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct PlayParam {
    /// Agent name from the config file.
    agent: String,
    /// What the agent transmits: `"silence"`, `"ausine,<freq>"` (a sine tone,
    /// e.g. `ausine,425`) or `"aufile,<path>"` (a mono WAV file).
    spec: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct WaitEventParam {
    /// Agent name from the config file.
    agent: String,
    /// How long to wait, in milliseconds (default 30000, max 120000).
    #[serde(default)]
    timeout_ms: Option<u64>,
    /// Only wait for this event (or one of these events), e.g. `"call_established"`
    /// or `["call_established", "call_closed"]` to await a call's outcome either
    /// way. Non-matching events are skipped. Omit to take the next event of any
    /// kind. Valid names are listed by an invalid value's error message.
    #[serde(default)]
    event: Option<EventFilter>,
}

/// A `wait_event` filter: one event name, or several (any of them matches).
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
enum EventFilter {
    /// One event name.
    One(String),
    /// Any of these event names.
    Any(Vec<String>),
}

impl EventFilter {
    fn names(&self) -> Vec<String> {
        match self {
            Self::One(n) => vec![n.clone()],
            Self::Any(v) => v.clone(),
        }
    }
}

/// The names `wait_event`'s filter accepts (== every `event_name` output).
const KNOWN_EVENT_NAMES: &[&str] = &[
    "registering",
    "register_ok",
    "register_failed",
    "unregistered",
    "call_incoming",
    "call_outgoing",
    "call_ringing",
    "call_established",
    "call_closed",
    "call_deflected",
    "call_hold",
    "call_resume",
    "call_transfer_failed",
    "voicemail_status",
    "response",
    "backend_connect_failed",
];

#[derive(Deserialize, schemars::JsonSchema)]
struct SaveAudioParam {
    /// Agent name from the config file.
    agent: String,
    /// File prefix; WAVs are written as `<prefix>-<call>-<tag>.wav`.
    prefix: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct StreamOpenParam {
    /// Agent name from the config file.
    agent: String,
    /// What the stream carries: `"rx"` (agent → you), `"tx"` (you → agent)
    /// or `"duplex"`.
    mode: String,
    /// Sample rate you will SEND audio at, in Hz (rx/duplex). Default 16000.
    #[serde(default)]
    tx_rate: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct StreamCloseParam {
    /// Stream id from `stream_open`.
    stream_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct CallHeadersParam {
    /// Agent name from the config file.
    agent: String,
    /// Optional SIP Call-ID (as reported by `call_incoming`/`agent_status`):
    /// return only that call's headers. Omit for all known calls (newest
    /// first).
    call_id: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct AddHeaderParam {
    /// Agent name from the config file.
    agent: String,
    /// Header name (e.g. `X-Session-Tag`).
    name: String,
    /// Header value. Templates are rendered once, NOW: `${uuid}` becomes a
    /// fresh identifier in this call (it will NOT re-render on future calls —
    /// declare per-call templates in the config file instead).
    value: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct RmHeaderParam {
    /// Agent name from the config file.
    agent: String,
    /// Header name; ALL headers with this name are removed.
    name: String,
}

// Helpers ───────────────────────────────────────────────────────────────────

fn ok_text(msg: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(msg)]))
}

fn ok_json(v: serde_json::Value) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(
        v.to_string(),
    )]))
}

fn invalid(msg: impl Into<String>) -> McpError {
    McpError::invalid_params(msg.into(), None)
}

/// Compact JSON for one event, for `wait_event`.
/// The `event`-field name for an [`AppEvent`] — the single naming source for
/// `event_json`, `wait_event`'s optional filter and the WS bridge's pushes.
pub fn event_name(e: &ringo_core::event::AppEvent) -> &'static str {
    use ringo_core::event::AppEvent::*;
    match e {
        Registering { .. } => "registering",
        RegisterOk { .. } => "register_ok",
        RegisterFailed { .. } => "register_failed",
        Unregistered { .. } => "unregistered",
        CallIncoming { .. } => "call_incoming",
        CallOutgoing { .. } => "call_outgoing",
        CallRinging { .. } => "call_ringing",
        CallEstablished { .. } => "call_established",
        CallClosed { .. } => "call_closed",
        CallDeflected { .. } => "call_deflected",
        CallHold { .. } => "call_hold",
        CallResume { .. } => "call_resume",
        CallTransferFailed { .. } => "call_transfer_failed",
        VoicemailStatus { .. } => "voicemail_status",
        Response { .. } => "response",
        Unknown { .. } => "unknown",
        BackendConnectFailed { .. } => "backend_connect_failed",
    }
}

/// Compact JSON for one event, for `wait_event` (and, with the `event` key
/// renamed to `type`, the WS bridge's text-frame pushes).
pub(crate) fn event_json(e: &ringo_core::event::AppEvent) -> serde_json::Value {
    use ringo_core::event::AppEvent::*;
    match e {
        Registering { account } => json!({"event": "registering", "account": account}),
        RegisterOk { account } => json!({"event": "register_ok", "account": account}),
        RegisterFailed { reason } => json!({"event": "register_failed", "reason": reason}),
        Unregistered { account } => json!({"event": "unregistered", "account": account}),
        CallIncoming {
            call_id,
            number,
            display_name,
        } => json!({
            "event": "call_incoming",
            "call_id": call_id,
            "from": number,
            "display_name": display_name,
        }),
        CallOutgoing { call_id, number } => {
            json!({"event": "call_outgoing", "call_id": call_id, "to": number})
        }
        CallRinging { call_id } => json!({"event": "call_ringing", "call_id": call_id}),
        CallEstablished { call_id } => {
            json!({"event": "call_established", "call_id": call_id})
        }
        CallClosed {
            call_id,
            reason,
            error,
        } => json!({
            "event": "call_closed",
            "call_id": call_id,
            "reason": reason,
            "error": error,
        }),
        CallDeflected {
            from,
            display_name,
            target,
        } => json!({
            "event": "call_deflected",
            "from": from,
            "display_name": display_name,
            "target": target,
        }),
        CallHold { call_id } => json!({"event": "call_hold", "call_id": call_id}),
        CallResume { call_id } => json!({"event": "call_resume", "call_id": call_id}),
        CallTransferFailed { call_id } => {
            json!({"event": "call_transfer_failed", "call_id": call_id})
        }
        VoicemailStatus { waiting, new_count } => {
            json!({"event": "voicemail_status", "waiting": waiting, "new_count": new_count})
        }
        Response { ok, data } => json!({"event": "response", "ok": ok, "data": data}),
        // Never surfaces through wait_event/WS (the hub filters all Unknown),
        // kept for direct event_json callers — deliberately WITHOUT the raw
        // backend class/type numbers: backend details don't leak here.
        Unknown { .. } => json!({"event": "unknown"}),
        BackendConnectFailed { reason } => {
            json!({"event": "backend_connect_failed", "reason": reason})
        }
    }
}

// Tools ─────────────────────────────────────────────────────────────────────

#[tool_router(router = build_tool_router)]
impl TelephonyServer {
    #[tool(
        description = "List all configured telephony agents (name, SIP address, registration, live calls). Agents start lazily: `running: false` means the agent is configured but its worker will be started by the first tool call that uses it."
    )]
    fn list_agents(&self) -> Result<CallToolResult, McpError> {
        let agents: Vec<serde_json::Value> = self
            .hub
            .overview()
            .into_iter()
            .map(|a| {
                json!({
                    "name": a.name,
                    "aor": a.aor,
                    "running": a.running,
                    "registered": a.state.as_ref().map(|s| s.registered),
                    "reg_error": a.state.as_ref().and_then(|s| s.reg_error.clone()),
                    "worker_dead": a.state.as_ref().map(|s| s.worker_dead),
                    "calls": a.state.map(|s| s.calls).unwrap_or_default(),
                })
            })
            .collect();
        ok_json(json!({ "agents": agents }))
    }

    #[tool(
        description = "Detailed state of one agent: registration, live calls (id, phase, remote party), media quality, received DTMF. Starts the agent's worker process if this is its first use. Poll this after dial/accept to check progress."
    )]
    async fn agent_status(
        &self,
        Parameters(AgentParam { agent }): Parameters<AgentParam>,
    ) -> Result<CallToolResult, McpError> {
        let a = self.agent(&agent).await?;
        let s = a.state();
        // The worker queries block on a std channel (bounded by the client's
        // query timeout) — keep them off the async runtime's threads.
        let (stats, call_count, received_dtmf) = {
            let a = Arc::clone(&a);
            tokio::task::spawn_blocking(move || {
                (a.media_stats(), a.call_count(), a.received_dtmf())
            })
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };
        let stats = stats.map(|m| {
            json!({
                "rtt_ms": m.rtt_ms,
                "jitter_ms": m.jitter_ms,
                "rx_lost": m.rx_lost,
                "packet_loss_pct": m.packet_loss_pct,
                "mos": m.mos,
            })
        });
        ok_json(json!({
            "name": a.name,
            "aor": a.aor,
            "registered": s.registered,
            "reg_error": s.reg_error,
            "worker_dead": s.worker_dead,
            "calls": s.calls,
            "call_count": call_count,
            "media_stats": stats,
            "received_dtmf": received_dtmf,
            "last_call_reason": s.last_call_reason,
            "last_call_error": s.last_call_error,
        }))
    }

    #[tool(
        description = "Place an outgoing call from an agent. Starts the agent's worker process if this is its first use. Fire-and-forget: returns immediately; wait for `call_ringing`/`call_established`/`call_closed` via `wait_event`, or poll `agent_status`."
    )]
    async fn dial(
        &self,
        Parameters(DialParam { agent, target }): Parameters<DialParam>,
    ) -> Result<CallToolResult, McpError> {
        let a = self.agent(&agent).await?;
        let resolved = self.hub.check_dial(&target, &a.domain).map_err(invalid)?;
        a.dial(&resolved);
        ok_text(format!("dialing `{target}` from `{agent}`"))
    }

    #[tool(
        description = "Accept the currently ringing (incoming) call of an agent. Starts the agent's worker process if this is its first use."
    )]
    async fn accept(
        &self,
        Parameters(AgentParam { agent }): Parameters<AgentParam>,
    ) -> Result<CallToolResult, McpError> {
        self.agent(&agent).await?.accept();
        ok_text(format!("accepting current call on `{agent}`"))
    }

    #[tool(description = "Hang up the current call of an agent.")]
    async fn hangup(
        &self,
        Parameters(AgentParam { agent }): Parameters<AgentParam>,
    ) -> Result<CallToolResult, McpError> {
        self.agent(&agent).await?.hangup();
        ok_text(format!("hanging up current call on `{agent}`"))
    }

    #[tool(description = "Hang up all calls of an agent.")]
    async fn hangup_all(
        &self,
        Parameters(AgentParam { agent }): Parameters<AgentParam>,
    ) -> Result<CallToolResult, McpError> {
        self.agent(&agent).await?.hangup_all();
        ok_text(format!("hanging up all calls on `{agent}`"))
    }

    #[tool(description = "Put the current call of an agent on hold.")]
    async fn hold(
        &self,
        Parameters(AgentParam { agent }): Parameters<AgentParam>,
    ) -> Result<CallToolResult, McpError> {
        self.agent(&agent).await?.hold();
        ok_text(format!("holding current call on `{agent}`"))
    }

    #[tool(description = "Resume the held call of an agent.")]
    async fn resume(
        &self,
        Parameters(AgentParam { agent }): Parameters<AgentParam>,
    ) -> Result<CallToolResult, McpError> {
        self.agent(&agent).await?.resume();
        ok_text(format!("resuming held call on `{agent}`"))
    }

    #[tool(description = "Mute the agent's outgoing audio (the remote party hears silence).")]
    async fn mute(
        &self,
        Parameters(AgentParam { agent }): Parameters<AgentParam>,
    ) -> Result<CallToolResult, McpError> {
        self.agent(&agent).await?.mute();
        ok_text(format!("`{agent}` is muted"))
    }

    #[tool(description = "Send one DTMF digit (0-9, *, #, A-F) on the agent's current call.")]
    async fn send_dtmf(
        &self,
        Parameters(DtmfParam { agent, digit }): Parameters<DtmfParam>,
    ) -> Result<CallToolResult, McpError> {
        let d = digit.trim();
        let Some(d) = d.chars().next() else {
            return Err(invalid("digit must not be empty"));
        };
        if !matches!(d.to_ascii_uppercase(), '0'..='9' | '*' | '#' | 'A'..='F') {
            return Err(invalid(format!("`{d}` is not a DTMF digit")));
        }
        self.agent(&agent).await?.send_dtmf(d);
        ok_text(format!("sent DTMF `{d}` on `{agent}`"))
    }

    #[tool(description = "Blind-transfer the agent's current call to another SIP URI / number.")]
    async fn transfer(
        &self,
        Parameters(TransferParam { agent, target }): Parameters<TransferParam>,
    ) -> Result<CallToolResult, McpError> {
        let a = self.agent(&agent).await?;
        let resolved = self.hub.check_dial(&target, &a.domain).map_err(invalid)?;
        a.transfer(&resolved);
        ok_text(format!("transferring `{agent}`'s call to `{target}`"))
    }

    #[tool(
        description = "Set what an agent transmits into the call: `\"silence\"`, a sine tone like `\"ausine,425\"`, or a WAV file like `\"aufile,/path/to/mono.wav\"`. Headless agents are silent until you call this. Call-scoped: resets to silence when the agent's last call ends, so the next call won't replay it."
    )]
    async fn play(
        &self,
        Parameters(PlayParam { agent, spec }): Parameters<PlayParam>,
    ) -> Result<CallToolResult, McpError> {
        self.agent(&agent).await?.play(&spec);
        ok_text(format!("`{agent}` now transmits `{spec}`"))
    }

    #[tool(
        description = "Block until the agent's NEXT event (call_incoming, call_established, call_closed, register_failed, …), then return it as JSON. Optionally pass `event` (a name or an array) to wait for specific events only — others are skipped, e.g. [\"call_established\",\"call_closed\"] awaits a call's outcome either way. Returns `{\"timeout\": true}` if nothing happened in time. Use after dial/accept to observe progress."
    )]
    async fn wait_event(
        &self,
        Parameters(WaitEventParam {
            agent,
            timeout_ms,
            event,
        }): Parameters<WaitEventParam>,
    ) -> Result<CallToolResult, McpError> {
        let a = self.agent(&agent).await?;
        // Validate an optional filter up front: a typo must fail loudly, not
        // silently never match.
        let filter = event.map(|f| f.names());
        if let Some(names) = filter.as_deref() {
            for n in names {
                if !KNOWN_EVENT_NAMES.contains(&n.as_str()) {
                    return Err(invalid(format!(
                        "unknown event `{n}`; valid: {}",
                        KNOWN_EVENT_NAMES.join(", ")
                    )));
                }
            }
        }
        let ms = timeout_ms.unwrap_or(DEFAULT_WAIT_MS).min(MAX_WAIT_MS);
        let names: Option<Vec<&str>> = filter
            .as_deref()
            .map(|v| v.iter().map(String::as_str).collect());
        match a
            .wait_event(Duration::from_millis(ms), names.as_deref())
            .await
        {
            Some(e) => ok_json(event_json(&e)),
            None => ok_json(json!({"timeout": true})),
        }
    }

    #[tool(
        description = "Stop an agent's worker process: it deregisters from SIP and exits. Idempotent (`false` = it wasn't running). The agent starts again on its next use — stopping frees the registration/worker without disabling the agent."
    )]
    async fn agent_stop(
        &self,
        Parameters(AgentParam { agent }): Parameters<AgentParam>,
    ) -> Result<CallToolResult, McpError> {
        let stopped = self
            .hub
            .stop(&agent)
            .await
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        if stopped {
            ok_text(format!(
                "stopped agent `{agent}` (deregistering; it starts again on next use)"
            ))
        } else {
            ok_text(format!("agent `{agent}` is not running (nothing to stop)"))
        }
    }

    #[tool(
        description = "Write the agent's current-call audio (sent + received) to WAV files. Requires `record_audio = true` in `[backend]`; returns the created file paths."
    )]
    async fn save_audio(
        &self,
        Parameters(SaveAudioParam { agent, prefix }): Parameters<SaveAudioParam>,
    ) -> Result<CallToolResult, McpError> {
        let a = self.agent(&agent).await?;
        let paths = tokio::task::spawn_blocking(move || a.save_audio(&prefix))
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        if paths.is_empty() {
            return Err(McpError::internal_error(
                "no captured audio to save (no active/recorded call, or record_audio is off)",
                None,
            ));
        }
        ok_json(json!({ "files": paths }))
    }

    #[tool(
        description = "Open a live-audio WebSocket stream for an agent and return its URL — MCP can't stream, so raw PCM travels on the socket: binary frames are mono s16le PCM (received audio from the call, rate announced via an `rx_started` text frame; audio you send back at the given tx_rate), text frames are control JSON (`ping`/`pong`, `flush_tx` for barge-in, and the agent's call events pushed live). One connection per URL/token, token valid 300 s. This is the transport for STT/TTS pipelines and live listening."
    )]
    async fn stream_open(
        &self,
        Parameters(StreamOpenParam {
            agent,
            mode,
            tx_rate,
        }): Parameters<StreamOpenParam>,
    ) -> Result<CallToolResult, McpError> {
        let mode = crate::bridge::StreamMode::parse(&mode).map_err(invalid)?;
        let tx_rate = tx_rate.unwrap_or(16000);
        if !(8000..=48000).contains(&tx_rate) {
            return Err(invalid(format!(
                "tx_rate must be within 8000..48000 Hz, got {tx_rate}"
            )));
        }
        let info = self
            .hub
            .stream_open(&agent, mode, tx_rate)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        ok_json(json!({
            "url": info.url,
            "stream_id": info.stream_id,
            "mode": info.mode,
            "tx_rate": info.tx_rate,
            "token_ttl_s": info.token_ttl_s,
            "protocol": {
                "binary": "raw mono s16le PCM",
                "text": ["rx_started", "rx_lagged", "tx_flushed", "pong", "error", "<call events>"],
                "client_to_server": ["ping", "flush_tx"],
            },
        }))
    }

    #[tool(
        description = "Close a live-audio stream opened with `stream_open` (takes its stream_id)."
    )]
    async fn stream_close(
        &self,
        Parameters(StreamCloseParam { stream_id }): Parameters<StreamCloseParam>,
    ) -> Result<CallToolResult, McpError> {
        self.hub
            .stream_close(&stream_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        ok_text(format!("closed stream `{stream_id}`"))
    }

    #[tool(
        description = "Add a custom SIP header to the agent's outgoing INVITEs (persistent until removed or the worker restarts). The value is a template rendered once, now: `${uuid}` becomes a fresh identifier in this call and stays fixed for future calls — for a fresh uuid per call, declare it in ringo-mcp's config file. Removes any header of the same name first (baresip's uarmheader semantics)."
    )]
    async fn add_header(
        &self,
        Parameters(AddHeaderParam { agent, name, value }): Parameters<AddHeaderParam>,
    ) -> Result<CallToolResult, McpError> {
        self.agent(&agent).await?.add_header(&name, &value);
        ok_text(format!(
            "`{agent}` now sends `{name}: {value}` on outgoing INVITEs"
        ))
    }

    #[tool(
        description = "Remove ALL custom SIP headers with this name from the agent's outgoing INVITEs."
    )]
    async fn rm_header(
        &self,
        Parameters(RmHeaderParam { agent, name }): Parameters<RmHeaderParam>,
    ) -> Result<CallToolResult, McpError> {
        self.agent(&agent).await?.rm_header(&name);
        ok_text(format!(
            "removed header `{name}` from `{agent}`'s outgoing INVITEs"
        ))
    }

    #[tool(
        description = "SIP headers of received INVITEs (incoming calls), as `[[name, value], …]` pairs — order and duplicates preserved. Filter by `call_id`, or omit it for all known calls (newest first). Headers can lag `call_incoming` by ~150ms; if empty right after the event, retry once."
    )]
    async fn call_headers(
        &self,
        Parameters(CallHeadersParam { agent, call_id }): Parameters<CallHeadersParam>,
    ) -> Result<CallToolResult, McpError> {
        let a = self.agent(&agent).await?;
        let s = a.state();
        if let Some(call_id) = call_id.as_deref() {
            return match s.headers_of(call_id) {
                Some(headers) => ok_json(json!({ "call_id": call_id, "headers": headers })),
                None => Err(invalid(format!(
                    "no INVITE headers known for Call-ID `{call_id}`"
                ))),
            };
        }
        let calls: Vec<serde_json::Value> = s
            .received_headers
            .iter()
            .rev()
            .map(|(call_id, headers)| json!({ "call_id": call_id, "headers": headers }))
            .collect();
        ok_json(json!({ "calls": calls }))
    }
}

#[tool_handler(router = self.tool_router.clone())]
impl ServerHandler for TelephonyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("ringo-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Telephony over SIP. Agents are configured in ringo-mcp's config file; \
                 address them by name (they start lazily on first use). Typical flow: \
                 `list_agents` to see who is configured, `dial` (or wait for \
                 `call_incoming` via `wait_event`, then `accept`), then `wait_event` \
                 until `call_established`/`call_closed`. Use `send_dtmf`, `play`, \
                 `transfer`, `hold`/`resume`/`mute` during the call, and \
                 `agent_status` to poll state and media quality."
                    .to_string(),
            )
    }
}

/// Serve MCP over stdio until the client disconnects.
pub async fn serve(hub: Arc<Hub>, disabled_tools: Vec<String>) -> anyhow::Result<()> {
    let service = TelephonyServer::new(hub, disabled_tools)
        .serve(stdio())
        .await
        .map_err(|e| anyhow::anyhow!("serve MCP over stdio: {e}"))?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_call_flow_events_render_backend_neutral() {
        let v = event_json(&ringo_core::event::AppEvent::CallHold {
            call_id: "c1".into(),
        });
        assert_eq!(v, json!({"event": "call_hold", "call_id": "c1"}));
        let v = event_json(&ringo_core::event::AppEvent::CallResume {
            call_id: "c1".into(),
        });
        assert_eq!(v, json!({"event": "call_resume", "call_id": "c1"}));
        let v = event_json(&ringo_core::event::AppEvent::CallTransferFailed {
            call_id: "c1".into(),
        });
        assert_eq!(v, json!({"event": "call_transfer_failed", "call_id": "c1"}));
    }

    #[test]
    fn tool_flag_resolution() {
        let owned = |v: &[&str]| -> Vec<String> { v.iter().map(|s| s.to_string()).collect() };

        // Plain disables: a group expands, a tool name passes through.
        let d = resolve_disabled_tools(None, &owned(&["recording"])).unwrap();
        assert_eq!(d, vec!["save_audio"]);
        let d = resolve_disabled_tools(None, &owned(&["add_header"])).unwrap();
        assert_eq!(d, vec!["add_header"]);

        // Allowlist: everything not covered is disabled, disables subtract on top.
        let d = resolve_disabled_tools(Some(&owned(&["discovery"])), &[]).unwrap();
        assert_eq!(
            d.len(),
            total_tool_count() - 2,
            "total minus the discovery pair"
        );
        assert!(!d.contains(&"list_agents".to_string()));
        let d = resolve_disabled_tools(Some(&owned(&["discovery", "streams"])), &owned(&["dial"]))
            .unwrap();
        assert!(d.contains(&"dial".to_string()));
        assert!(!d.contains(&"stream_open".to_string()));
        assert!(!d.contains(&"agent_status".to_string()));

        // Unknown names fail loudly with the valid list.
        let err = resolve_disabled_tools(None, &owned(&["nope"]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown tool or group `nope`"), "{err}");
        assert!(err.contains("valid groups"), "{err}");
        let err = resolve_disabled_tools(Some(&owned(&["garbage"])), &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("garbage"), "{err}");

        // The group table covers every tool exactly once — no orphans, no dupes.
        let all = all_tool_names();
        let covered: Vec<&str> = TOOL_GROUPS
            .iter()
            .flat_map(|(_, tools)| tools.iter().copied())
            .collect();
        assert_eq!(all.len(), covered.len(), "duplicate tool in groups?");
    }

    #[test]
    fn event_names_match_event_json_and_the_known_list() {
        use ringo_core::event::AppEvent::*;
        // Every variant renders with exactly event_name()'s string, and
        // every wait_event filter name corresponds to a real variant.
        let cases: Vec<ringo_core::event::AppEvent> = vec![
            Registering {
                account: "a".into(),
            },
            RegisterOk {
                account: "a".into(),
            },
            RegisterFailed { reason: "r".into() },
            Unregistered {
                account: "a".into(),
            },
            CallIncoming {
                call_id: "c".into(),
                number: "n".into(),
                display_name: None,
            },
            CallOutgoing {
                call_id: "c".into(),
                number: "n".into(),
            },
            CallRinging {
                call_id: "c".into(),
            },
            CallEstablished {
                call_id: "c".into(),
            },
            CallClosed {
                call_id: "c".into(),
                reason: "r".into(),
                error: false,
            },
            CallDeflected {
                from: "f".into(),
                display_name: None,
                target: "t".into(),
            },
            CallHold {
                call_id: "c".into(),
            },
            CallResume {
                call_id: "c".into(),
            },
            CallTransferFailed {
                call_id: "c".into(),
            },
            VoicemailStatus {
                waiting: false,
                new_count: 0,
            },
            Response {
                ok: true,
                data: "d".into(),
            },
            Unknown {
                class: "x".into(),
                type_: "1".into(),
            },
            BackendConnectFailed { reason: "r".into() },
        ];
        let mut seen: Vec<&str> = Vec::new();
        for e in &cases {
            let rendered = event_json(e);
            let json_name = rendered["event"].as_str().expect("event field");
            assert_eq!(json_name, event_name(e));
            seen.push(event_name(e));
        }
        // The wait_event filter accepts exactly these (minus "unknown", which
        // the hub filters before it could ever match).
        let filterable: Vec<&str> = seen.into_iter().filter(|n| *n != "unknown").collect();
        assert_eq!(filterable, KNOWN_EVENT_NAMES);
    }

    #[test]
    fn unknown_renders_without_backend_details() {
        let v = event_json(&ringo_core::event::AppEvent::Unknown {
            class: "bevent".into(),
            type_: "29".into(),
        });
        assert_eq!(v, json!({"event": "unknown"}));
    }
}
