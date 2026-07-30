//! The `Agent` JS class — a thin handle that converts JS values and delegates to
//! the neutral [`crate::engine`] — plus the `agent(...)` global, its config
//! helpers, and the `info()` object builder.

use super::super::convert::{into_value, json_to_js, throw};
use super::audio::AudioSpec;
use super::core::HostState;
use crate::engine::assertion::Value as EngVal;
use crate::engine::audio;
use crate::engine::ctx::{CallState, Ctx as EngineCtx, sip_user_part};
use crate::engine::duration;
use indexmap::IndexMap;
use ringo_core::account::Account;
use rquickjs::class::Trace;
use rquickjs::function::Opt;
use rquickjs::{Class, Ctx as JsCtx, IntoJs, JsLifetime, Object, Result as JsResult, Value};
use std::sync::Arc;

// ── Agent-domain types (the JS-facing shapes live in the hand-written `.d.ts`) ──
/// The call states exposed to JS (`agent.state`). `#[derive(Enum)]` emits both the TS
/// `declare const enum State` and the [`State::VALUES`] pairs the runtime object is
/// built from — single source.
#[derive(ringo_flow_macros::TsEnum)]
pub(in crate::script::js) enum State {
    Idle,
    Ringing,
    Established,
}

impl State {
    /// The runtime `State` enum object (`{ Idle: "idle", … }`), built from
    /// [`State::VALUES`]. `install` wires it onto the global object.
    pub(in crate::script::js) fn object<'js>(ctx: &JsCtx<'js>) -> JsResult<Object<'js>> {
        let obj = Object::new(ctx.clone())?;
        for (key, value) in State::VALUES {
            obj.set(*key, *value)?;
        }
        Ok(obj)
    }

    /// The JS string value (`State.Ringing === "ringing"`).
    fn js_value(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Ringing => "ringing",
            State::Established => "established",
        }
    }
}

impl From<CallState> for State {
    fn from(s: CallState) -> Self {
        match s {
            CallState::Idle => State::Idle,
            CallState::Ringing => State::Ringing,
            CallState::Established => State::Established,
        }
    }
}

impl<'js> IntoJs<'js> for State {
    fn into_js(self, ctx: &JsCtx<'js>) -> JsResult<Value<'js>> {
        rquickjs::String::from_str(ctx.clone(), self.js_value()).map(Value::from_string)
    }
}
/// The current call's remote party, from the `peer` getter / `info()`. Built in
/// Rust and `into_js`-converted so the interface and the produced object share one
/// source.
#[derive(rquickjs::IntoJs, ringo_flow_macros::TsInterface)]
#[jsdoc(readonly)]
struct Peer {
    /// Full SIP URI of the remote party (e.g. `sip:bob@example.com`).
    uri: String,
    /// The remote party's number / user part.
    number: String,
    /// The remote party's display name, if the call signalled one.
    name: Option<String>,
}

/// RTP media quality of a call, from the `quality` getter. The whole object is
/// `undefined` until metrics are available (no RTCP yet); once present, every field
/// is a number — they arrive together, so there are no per-field nulls.
#[derive(rquickjs::IntoJs, ringo_flow_macros::TsInterface)]
#[jsdoc(readonly)]
struct CallQuality {
    /// Mean Opinion Score (1.0–5.0); higher is better.
    mos: f64,
    /// Round-trip time in milliseconds.
    rtt: f64,
    /// Jitter in milliseconds.
    jitter: f64,
    /// Fraction of packets lost (0.0–1.0).
    #[qjs(rename = "packetLoss")]
    packet_loss: f64,
}
/// The `agent(name, { … })` config. `#[derive(Interface)]` emits the TS `interface
/// AgentConfig` and the [`AgentConfig::FIELDS`] name list; [`AgentConfig::from_js`]
/// parses + validates a raw JS object into it, rejecting unknown keys against `FIELDS`.
#[derive(ringo_flow_macros::TsInterface)]
struct AgentConfig {
    /// SIP user (registration / auth). Required.
    username: String,
    /// SIP domain / registrar. Required.
    domain: String,
    /// Auth password.
    password: Option<String>,
    /// `udp` (default), `tcp` or `tls`.
    transport: Option<String>,
    /// auth user, if it differs from `username`.
    auth_user: Option<String>,
    /// caller display name.
    display_name: Option<String>,
    /// outbound proxy URI.
    outbound: Option<String>,
    /// STUN server, e.g. `stun:host:port`.
    stun_server: Option<String>,
    /// media encryption, e.g. `srtp`, `zrtp`, `dtls_srtp`.
    media_enc: Option<String>,
    /// re-registration interval (seconds); `0` disables.
    regint: Option<u32>,
    /// subscribe to message-waiting indication.
    mwi: Option<bool>,
    /// `"info"` for reliable headless DTMF (SIP INFO).
    dtmf_mode: Option<String>,
    /// extra SIP headers on the INVITE, e.g. `{ "X-Foo": "bar" }`.
    // `IndexMap` (order-preserving) maps to `Record<string, string>` automatically.
    headers: Option<IndexMap<String, String>>,
    /// deflect inbound calls with a 302 to this URI/number.
    deflect_to: Option<String>,
    /// free-form data carried on the agent, read back as `agent.metadata`
    /// (e.g. `{ role: "caller" }`); not used for SIP.
    #[jsdoc(optional, type = "Record<string, unknown>")]
    metadata: Option<serde_json::Value>,
}

impl AgentConfig {
    /// Parse + validate a raw JS config object: reject unknown keys (against
    /// [`AgentConfig::FIELDS`]), require `username`/`domain`, type-check each field
    /// (no silent coercion), and collect order-preserving, token-validated headers.
    fn from_js(label: &str, config: &Object<'_>) -> Result<Self, String> {
        super::super::bindings::reject_unknown_keys(label, config, Self::FIELDS)?;
        let headers = headers_from_obj(label, config)?;
        Ok(AgentConfig {
            username: cfg_str(label, config, "username")?
                .ok_or_else(|| format!("{label}: `username` is required"))?,
            domain: cfg_str(label, config, "domain")?
                .ok_or_else(|| format!("{label}: `domain` is required"))?,
            password: cfg_str(label, config, "password")?,
            transport: cfg_str(label, config, "transport")?,
            auth_user: cfg_str(label, config, "auth_user")?,
            display_name: cfg_str(label, config, "display_name")?,
            outbound: cfg_str(label, config, "outbound")?,
            stun_server: cfg_str(label, config, "stun_server")?,
            media_enc: cfg_str(label, config, "media_enc")?,
            regint: cfg_u32(label, config, "regint")?,
            mwi: cfg_bool(label, config, "mwi")?,
            dtmf_mode: cfg_str(label, config, "dtmf_mode")?,
            headers: Some(headers).filter(|h| !h.is_empty()),
            deflect_to: cfg_str(label, config, "deflect_to")?,
            metadata: metadata_from_obj(label, config)?,
        })
    }

    /// Split into the engine `Account` + INVITE headers + an optional declared
    /// deflection target + free-form metadata (the latter three aren't `Account`
    /// fields). Every `Account`
    /// field is set explicitly (no `..Default::default()`), so a new field is a
    /// compile error here rather than a silent omission.
    fn into_parts(self) -> (Account, Vec<(String, String)>, Option<String>, serde_json::Value) {
        let account = Account {
            username: self.username,
            domain: self.domain,
            password: self.password.unwrap_or_default(),
            display_name: self.display_name,
            transport: self.transport,
            auth_user: self.auth_user,
            outbound: self.outbound,
            stun_server: self.stun_server,
            media_enc: self.media_enc,
            regint: self.regint,
            mwi: self.mwi.unwrap_or(false),
            dtmf_mode: self.dtmf_mode,
            // In-process agents keep baresip's default routing (the process-per-agent
            // backend sets catchall itself); codec selection isn't a JS config key yet.
            catchall: false,
            audio_codecs: Vec::new(),
        };
        let headers = self.headers.map(|m| m.into_iter().collect()).unwrap_or_default();
        let metadata = self.metadata.unwrap_or_else(|| serde_json::Value::Object(Default::default()));
        (account, headers, self.deflect_to, metadata)
    }
}
/// A snapshot of an agent's observable state, from `agent.info()`. Renamed to the
/// `AgentInfo` interface in TS (the Rust name `AgentInfo` is the engine type). Built
/// in Rust and `into_js`-converted so the interface and the object share one source.
#[derive(rquickjs::IntoJs, ringo_flow_macros::TsInterface)]
#[jsdoc(rename = "AgentInfo")]
struct AgentSnapshot {
    /// The agent's name (as passed to `agent(name, …)`).
    name: String,
    /// The agent's address-of-record (`sip:user@domain`).
    aor: String,
    /// Whether the agent is currently registered.
    registered: bool,
    /// Current call phase (compare against `State.*`).
    #[jsdoc(type = "State")]
    state: String,
    /// SIP reason phrase of the last response, if any.
    reason: Option<String>,
    /// SIP status code of the last response, if any.
    #[qjs(rename = "statusCode")]
    status_code: Option<i64>,
    /// The current call's remote party, if there is a call.
    peer: Option<Peer>,
    /// Number of active calls on this agent.
    calls: i64,
}

/// A cheap handle to an agent: its name plus the shared neutral context. Mirrors
/// the rhai `Agent` handle — all state lives in `EngineCtx`, keyed by name.
#[derive(Trace, JsLifetime, Clone)]
#[rquickjs::class(rename = "Agent")]
pub struct Agent {
    #[qjs(skip_trace)]
    pub name: String,
    #[qjs(skip_trace)]
    pub ctx: Arc<EngineCtx>,
    #[qjs(skip_trace)]
    /// Free-form `agent(...)` metadata, carried on the handle (mirrors rhai).
    pub metadata: Arc<serde_json::Value>,
}

#[ringo_flow_macros::ts_export]
#[rquickjs::methods]
impl Agent {
    // ── getters ──
    #[qjs(get)]
    fn registered<'js>(&self, ctx: JsCtx<'js>) -> JsResult<bool> {
        self.ctx.registered(&self.name).map_err(|e| throw(&ctx, &e))
    }

    #[qjs(get)]
    fn state<'js>(&self, ctx: JsCtx<'js>) -> JsResult<State> {
        self.ctx.call_state(&self.name).map(State::from).map_err(|e| throw(&ctx, &e))
    }

    /// RTP media quality of the active/last call (`{ mos, rtt, jitter, packetLoss }`),
    /// or `undefined` until metrics are available (no RTCP report yet).
    #[qjs(get)]
    fn quality<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Option<CallQuality>> {
        let stats = self.ctx.quality(&self.name).map_err(|e| throw(&ctx, &e))?;
        crate::engine::ctx::mark_pending_label(format!("{} quality MOS", self.name));
        Ok(stats.map(|s| CallQuality {
            mos: s.mos,
            rtt: s.rtt_ms,
            jitter: s.jitter_ms,
            packet_loss: s.packet_loss_pct,
        }))
    }

    #[qjs(get, rename = "receivedDtmf")]
    fn received_dtmf<'js>(&self, ctx: JsCtx<'js>) -> JsResult<String> {
        self.ctx.received_dtmf(&self.name).map_err(|e| throw(&ctx, &e))
    }

    /// Free-form metadata attached at `agent(...)` via the `metadata` config field
    /// (e.g. `caller.metadata.role`). Empty object if none was given.
    #[qjs(get)]
    #[jsdoc(type = "Record<string, unknown>")]
    fn metadata<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Value<'js>> {
        json_to_js(&ctx, self.metadata.as_ref())
    }

    // ── call control ──
    fn register<'js>(&self, ctx: JsCtx<'js>) -> JsResult<()> {
        self.ctx.register(&self.name).map_err(|e| throw(&ctx, &e))
    }
    fn accept<'js>(&self, ctx: JsCtx<'js>) -> JsResult<()> {
        self.ctx.accept(&self.name).map_err(|e| throw(&ctx, &e))
    }
    fn hangup<'js>(&self, ctx: JsCtx<'js>) -> JsResult<()> {
        self.ctx.hangup(&self.name).map_err(|e| throw(&ctx, &e))
    }
    /// `dtmf(digits)` sends back-to-back; `dtmf(digits, gap)` inserts a pause
    /// (e.g. `"200ms"`) between digits.
    fn dtmf<'js>(&self, ctx: JsCtx<'js>, digits: String, gap: Opt<String>) -> JsResult<()> {
        let gap = match gap.0 {
            Some(s) => duration::parse_duration(&s).map_err(|e| throw(&ctx, &e))?,
            None => std::time::Duration::ZERO,
        };
        self.ctx
            .dtmf(&self.name, &digits, gap)
            .map_err(|e| throw(&ctx, &e))
    }
    /// Dial a target: another `Agent` (at its AOR) or a SIP URI / number string.
    fn dial<'js>(&self, ctx: JsCtx<'js>, #[jsdoc(type = "Agent | string")] target: Value<'js>) -> JsResult<()> {
        dispatch_target(&ctx, "dial", &target, |n| self.ctx.dial_agent(&self.name, n), |u| {
            self.ctx.dial_uri(&self.name, u)
        })
    }
    fn hold<'js>(&self, ctx: JsCtx<'js>) -> JsResult<()> {
        self.ctx.hold(&self.name).map_err(|e| throw(&ctx, &e))
    }
    fn resume<'js>(&self, ctx: JsCtx<'js>) -> JsResult<()> {
        self.ctx.resume(&self.name).map_err(|e| throw(&ctx, &e))
    }
    fn mute<'js>(&self, ctx: JsCtx<'js>) -> JsResult<()> {
        self.ctx.mute(&self.name).map_err(|e| throw(&ctx, &e))
    }

    // ── transfers ──
    /// Blind-transfer the current call to a target: another `Agent` or a URI string.
    fn transfer<'js>(&self, ctx: JsCtx<'js>, #[jsdoc(type = "Agent | string")] target: Value<'js>) -> JsResult<()> {
        dispatch_target(&ctx, "transfer", &target, |n| self.ctx.transfer_agent(&self.name, n), |u| {
            self.ctx.transfer_uri(&self.name, u)
        })
    }
    /// Start an attended transfer to a target: another `Agent` or a URI string.
    #[qjs(rename = "attendedTransfer")]
    fn attended_transfer<'js>(&self, ctx: JsCtx<'js>, #[jsdoc(type = "Agent | string")] target: Value<'js>) -> JsResult<()> {
        dispatch_target(&ctx, "attendedTransfer", &target, |n| self.ctx.attended_transfer_agent(&self.name, n), |u| {
            self.ctx.attended_transfer_uri(&self.name, u)
        })
    }
    #[qjs(rename = "completeTransfer")]
    fn complete_transfer<'js>(&self, ctx: JsCtx<'js>) -> JsResult<()> {
        self.ctx
            .complete_transfer(&self.name)
            .map_err(|e| throw(&ctx, &e))
    }
    #[qjs(rename = "abortTransfer")]
    fn abort_transfer<'js>(&self, ctx: JsCtx<'js>) -> JsResult<()> {
        self.ctx
            .abort_transfer(&self.name)
            .map_err(|e| throw(&ctx, &e))
    }

    // ── deflection ──
    /// Deflect inbound calls (302) to a target: another `Agent` or a URI / number string.
    fn deflect<'js>(&self, ctx: JsCtx<'js>, #[jsdoc(type = "Agent | string")] target: Value<'js>) -> JsResult<()> {
        dispatch_target(&ctx, "deflect", &target, |n| self.ctx.deflect_to_agent(&self.name, n), |u| {
            self.ctx.deflect_to_uri(&self.name, u)
        })
    }
    #[qjs(rename = "stopDeflect")]
    fn stop_deflect<'js>(&self, ctx: JsCtx<'js>) -> JsResult<()> {
        self.ctx
            .stop_deflect(&self.name)
            .map_err(|e| throw(&ctx, &e))
    }
    /// Answer inbound INVITEs with a custom SIP response instead of accepting.
    /// `respondIncoming(486, "Busy Here")`, or with extra header lines:
    /// `respondIncoming(302, "Moved Temporarily", { Contact: "<sip:bob@example.com>" })`.
    #[qjs(rename = "respondIncoming")]
    fn respond_incoming<'js>(
        &self,
        ctx: JsCtx<'js>,
        code: i64,
        reason: String,
        #[jsdoc(type = "Record<string, string>")] headers: Opt<Object<'js>>,
    ) -> JsResult<()> {
        // The engine takes full header lines (`"Name: value"`); build them from the
        // optional `{ Name: value }` object.
        let lines = match headers.0 {
            Some(h) => h
                .props::<String, String>()
                .filter_map(JsResult::ok)
                .map(|(k, v)| format!("{k}: {v}"))
                .collect(),
            None => Vec::new(),
        };
        self.ctx
            .respond_incoming(&self.name, code as u16, &reason, lines)
            .map_err(|e| throw(&ctx, &e))
    }
    /// A snapshot of the agent's observable state as an object. (For a JSON string,
    /// just `JSON.stringify(agent.info())`.)
    #[jsdoc(type = "AgentInfo")]
    fn info<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Value<'js>> {
        let i = self.ctx.info(&self.name).map_err(|e| throw(&ctx, &e))?;
        info_object(&ctx, &i)
    }

    // ── additional getters ──
    #[qjs(get)]
    fn reason<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Option<String>> {
        self.ctx.reason(&self.name).map_err(|e| throw(&ctx, &e))
    }
    #[qjs(get, rename = "statusCode")]
    fn status_code<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Option<i64>> {
        Ok(self.ctx.status_code(&self.name).map_err(|e| throw(&ctx, &e))?.map(|c| c as i64))
    }
    /// First value of a received INVITE header (`a.header("X-Trace-Id")`).
    fn header<'js>(&self, ctx: JsCtx<'js>, name: String) -> JsResult<Option<String>> {
        self.ctx.header(&self.name, &name).map_err(|e| throw(&ctx, &e))
    }
    /// All received INVITE headers as a `{ name: value }` object.
    #[qjs(get)]
    #[jsdoc(type = "Record<string, string>")]
    fn headers<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Value<'js>> {
        match self.ctx.headers(&self.name) {
            Ok(pairs) => into_value(
                &ctx,
                EngVal::Map(pairs.into_iter().map(|(k, v)| (k, EngVal::Str(v))).collect()),
            ),
            Err(e) => Err(throw(&ctx, &e)),
        }
    }
    /// The current call's remote party: `{ uri, number, name }`, or `undefined`.
    #[qjs(get)]
    fn peer<'js>(&self, ctx: JsCtx<'js>) -> JsResult<Option<Peer>> {
        Ok(self.ctx.peer(&self.name).map_err(|e| throw(&ctx, &e))?.map(|(uri, name)| Peer {
            number: sip_user_part(&uri),
            uri,
            name,
        }))
    }

    // ── audio ──
    /// Set this agent's audio source on the active call (`a.sendAudio(tone(440))`).
    #[qjs(rename = "sendAudio")]
    fn send_audio<'js>(&self, ctx: JsCtx<'js>, spec: Class<'js, AudioSpec>) -> JsResult<()> {
        let spec = spec.borrow().inner.clone();
        audio::send_audio(&self.ctx, &self.name, spec).map_err(|e| throw(&ctx, &e))
    }
    /// Assert the agent receives a `freq` Hz tone within `within` (e.g. `"5s"`).
    /// Returns a Promise: the blocking detection window runs on the runtime's
    /// blocking pool, so `await Promise.all([a.verifyAudio(...), b.verifyAudio(...)])`
    /// listens on several agents concurrently instead of serially.
    #[qjs(rename = "verifyAudio")]
    async fn verify_audio<'js>(&self, ctx: JsCtx<'js>, freq: i64, within: String) -> JsResult<()> {
        let eng = self.ctx.clone();
        let name = self.name.clone();
        let handle = self.ctx.rt.clone();
        match handle
            .spawn_blocking(move || audio::verify_audio(&eng, &name, freq, &within))
            .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(throw(&ctx, &e)),
            Err(e) => Err(throw(&ctx, &format!("verifyAudio task failed: {e}"))),
        }
    }
}

/// Connect a headless baresip agent and return a handle.
#[ringo_flow_macros::ts_global(name = "agent")]
pub(in crate::script::js) fn agent_global<'js>(
    cx: JsCtx<'js>,
    name: String,
    #[jsdoc(type = "AgentConfig")] config: Object<'js>,
) -> rquickjs::Result<Class<'js, Agent>> {
    let eng = cx.userdata::<HostState>().expect("host state stored at install").eng.clone();
    let label = format!("agent `{name}`");
    let to_err = |e: String| throw(&cx, &e);
    let (account, headers, deflect_to, metadata) =
        AgentConfig::from_js(&label, &config).map_err(to_err)?.into_parts();
    eng.connect_agent(&name, account, &headers).map_err(to_err)?;
    // Arm declared deflection right after connect, before any inbound call.
    if let Some(target) = deflect_to {
        eng.deflect_to_uri(&name, &target).map_err(to_err)?;
    }
    Class::instance(cx, Agent { name, ctx: eng, metadata: Arc::new(metadata) })
}

/// Dispatch a dial/transfer/deflect target — an `Agent` handle or a SIP URI / number
/// string — to the matching engine call (one overloaded verb, mirroring rhai). The
/// closures receive the resolved agent name / URI string.
fn dispatch_target<'js>(
    ctx: &JsCtx<'js>,
    verb: &str,
    target: &Value<'js>,
    on_agent: impl FnOnce(&str) -> Result<(), String>,
    on_uri: impl FnOnce(&str) -> Result<(), String>,
) -> JsResult<()> {
    if let Some(s) = target.as_string() {
        on_uri(&s.to_string()?).map_err(|e| throw(ctx, &e))
    } else if let Ok(other) = Class::<Agent>::from_value(target) {
        let name = other.borrow().name.clone();
        on_agent(&name).map_err(|e| throw(ctx, &e))
    } else {
        Err(throw(ctx, &format!("{verb}: target must be an Agent or a SIP URI/number string")))
    }
}

/// Read an optional string field: absent/`null`/`undefined` → `None`, present but
/// not a string → a typed error (rather than silently coercing).
fn cfg_str(label: &str, config: &Object<'_>, key: &str) -> Result<Option<String>, String> {
    let v: Value = config
        .get(key)
        .map_err(|_| format!("{label}: `{key}` is unreadable"))?;
    if v.is_undefined() || v.is_null() {
        return Ok(None);
    }
    v.as_string()
        .and_then(|s| s.to_string().ok())
        .map(Some)
        .ok_or_else(|| format!("{label}: `{key}` must be a string"))
}

/// Read an optional non-negative integer field (present but not a number → error).
fn cfg_u32(label: &str, config: &Object<'_>, key: &str) -> Result<Option<u32>, String> {
    let v: Value = config
        .get(key)
        .map_err(|_| format!("{label}: `{key}` is unreadable"))?;
    if v.is_undefined() || v.is_null() {
        return Ok(None);
    }
    let n = v
        .as_int()
        .map(|i| i as f64)
        .or_else(|| v.as_float())
        .ok_or_else(|| format!("{label}: `{key}` must be a number"))?;
    if n < 0.0 || n.fract() != 0.0 {
        return Err(format!("{label}: `{key}` must be a non-negative integer"));
    }
    Ok(Some(n as u32))
}

/// Read an optional boolean field (present but not a boolean → error).
fn cfg_bool(label: &str, config: &Object<'_>, key: &str) -> Result<Option<bool>, String> {
    let v: Value = config
        .get(key)
        .map_err(|_| format!("{label}: `{key}` is unreadable"))?;
    if v.is_undefined() || v.is_null() {
        return Ok(None);
    }
    v.as_bool()
        .map(Some)
        .ok_or_else(|| format!("{label}: `{key}` must be a boolean"))
}

/// The optional `metadata` config object — free-form data carried on the agent
/// handle and read back as `agent.metadata`. Absent/`null`/`undefined` → `None`;
/// present but not a plain object → a typed error (mirrors rhai's `metadata_from_map`).
fn metadata_from_obj(label: &str, config: &Object<'_>) -> Result<Option<serde_json::Value>, String> {
    let val: Value = config
        .get("metadata")
        .map_err(|_| format!("{label}: `metadata` is unreadable"))?;
    if val.is_undefined() || val.is_null() {
        return Ok(None);
    }
    if !val.is_object() || val.is_array() {
        return Err(format!("{label}: `metadata` must be an object"));
    }
    let json = val
        .ctx()
        .json_stringify(val.clone())
        .ok()
        .flatten()
        .and_then(|s| s.to_string().ok())
        .ok_or_else(|| format!("{label}: `metadata` is not JSON-serialisable"))?;
    let parsed = serde_json::from_str(&json)
        .map_err(|_| format!("{label}: `metadata` is not JSON-serialisable"))?;
    Ok(Some(parsed))
}

/// `headers: { "X-Foo": "bar" }` → ordered `(name, value)` pairs, names validated as
/// SIP tokens (so they can't malform the backend's `uaaddheader`). Mirrors rhai.
fn headers_from_obj(label: &str, config: &Object<'_>) -> Result<IndexMap<String, String>, String> {
    let Some(h) = config
        .get::<_, Option<Object>>("headers")
        .map_err(|_| format!("{label}: `headers` must be an object"))?
    else {
        return Ok(IndexMap::new());
    };
    let mut out = IndexMap::new();
    for entry in h.props::<String, String>() {
        let (k, v) = entry.map_err(|_| format!("{label}: header values must be strings"))?;
        if !is_header_token(&k) {
            return Err(format!("{label}: `{k}` is not a valid SIP header name"));
        }
        out.insert(k, v);
    }
    Ok(out)
}

/// A valid SIP header name (token chars only) — no CRLF/space/`:` that could
/// malform the `uaaddheader` command.
fn is_header_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-.!%*_+`'~".contains(&b))
}

/// An [`AgentInfo`](crate::engine::AgentInfo) snapshot → a JS object (camelCase keys,
/// matching the `Agent` getters): `{ name, aor, registered, state, reason?,
/// statusCode?, peer?, calls }`. Built as an [`AgentSnapshot`] and `into_js`-ed, so
/// the produced shape and the `AgentInfo` interface share one source. Backs
/// `info()`.
fn info_object<'js>(
    ctx: &JsCtx<'js>,
    i: &crate::engine::AgentInfo,
) -> JsResult<Value<'js>> {
    let snapshot = AgentSnapshot {
        name: i.name.clone(),
        aor: i.aor.clone(),
        registered: i.registered,
        state: i.state.to_string(),
        reason: i.reason.clone(),
        status_code: i.status_code.map(|c| c as i64),
        peer: i.peer.as_ref().map(|(uri, name)| Peer {
            number: sip_user_part(uri),
            uri: uri.clone(),
            name: name.clone(),
        }),
        calls: i.calls as i64,
    };
    snapshot.into_js(ctx)
}
