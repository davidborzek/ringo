//! The live-audio WebSocket bridge: a small `ws://` server (loopback only)
//! that streams an agent's call audio to out-of-process consumers, next to the
//! MCP stdio control plane — MCP (JSON-RPC over stdio) can't push or stream, so
//! raw PCM gets its own channel.
//!
//! MCP stays the control plane: `stream_open` mints a one-shot, TTL-bound
//! token bound to one agent and returns the URL; the consumer then talks to
//! the socket directly.
//!
//! Wire protocol on one connection:
//! - **Binary frames** = raw mono s16le PCM. Server→client: the agent's
//!   received audio. Client→server: audio to transmit into the call (the
//!   client must send at the `tx_rate` from `stream_open`).
//! - **Text frames** = control JSON (both directions), tagged with `"type"`.
//!   Server→client: `rx_started` (announces the RX rate before the first
//!   binary frame), `rx_lagged` (frames dropped because the consumer fell
//!   behind), `tx_flushed`, `pong`, `error`, and the agent's events as
//!   `{"type": "<event>", …}` (same shape as `wait_event` results, so
//!   consumers can share parsing).
//!   Client→server: `ping`, `flush_tx` (barge-in: drops whatever was still
//!   queued for the worker and re-arms the stream).
//!
//! Ordering: the agent's TX path is a single writer thread (see
//! [`crate::hub::TxMsg`]), so the arming command always precedes audio and a
//! `flush_tx` can't interleave with in-flight samples.

use crate::hub::{Hub, TxMsg};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;

/// How long a minted token may sit unused before it expires.
pub(crate) const TOKEN_TTL: Duration = Duration::from_secs(300);
/// URL path prefix under which stream tokens are accepted.
const TOKEN_PATH_PREFIX: &str = "/s/";

/// What a stream carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamMode {
    /// Agent → client only (e.g. feeding an STT pipeline).
    Rx,
    /// Client → agent only (e.g. a TTS producer).
    Tx,
    /// Both directions.
    Duplex,
}

impl StreamMode {
    /// Parse the `mode` tool argument.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "rx" => Ok(Self::Rx),
            "tx" => Ok(Self::Tx),
            "duplex" => Ok(Self::Duplex),
            other => Err(format!("mode must be one of rx/tx/duplex, got `{other}`")),
        }
    }

    /// The wire name (also used in the `stream_open` reply).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rx => "rx",
            Self::Tx => "tx",
            Self::Duplex => "duplex",
        }
    }
}

/// The reply of the `stream_open` tool: where and how to connect.
#[derive(Debug, Clone, Serialize)]
pub struct StreamInfo {
    /// The `ws://` URL, token included. One connection per token.
    pub url: String,
    /// The stream id (`stream_close` takes this; equals the token).
    pub stream_id: String,
    /// What the stream carries.
    pub mode: &'static str,
    /// The sample rate the client must SEND at (tx/duplex). RX rate arrives as
    /// an `rx_started` control frame before the first binary frame.
    pub tx_rate: u32,
    /// Seconds the token stays valid if unused.
    pub token_ttl_s: u64,
}

/// A minted, not-yet-used stream authorization.
struct Grant {
    agent: String,
    mode: StreamMode,
    tx_rate: u32,
    expires: Instant,
}
/// Shared bridge state: the listener address, the pending grants and the kill
/// switches of live connections (both keyed by token).
pub struct BridgeState {
    addr: SocketAddr,
    grants: Mutex<HashMap<String, Grant>>,
    active: Mutex<HashMap<String, mpsc::UnboundedSender<()>>>,
}

impl BridgeState {
    pub(crate) fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            grants: Mutex::new(HashMap::new()),
            active: Mutex::new(HashMap::new()),
        }
    }

    /// Mint a fresh token for `agent`, dropping expired leftovers.
    pub(crate) async fn mint_grant(
        &self,
        agent: String,
        mode: StreamMode,
        tx_rate: u32,
    ) -> Result<StreamInfo> {
        // Two concatenated v4 UUIDs ≈ 244 random bits — no sequential RNG to
        // guess, no extra dependency.
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let mut grants = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        grants.retain(|_, g| g.expires > now);
        grants.insert(
            token.clone(),
            Grant {
                agent,
                mode,
                tx_rate,
                expires: now + TOKEN_TTL,
            },
        );
        Ok(StreamInfo {
            url: format!("ws://{}/s/{token}", self.addr),
            stream_id: token,
            mode: mode.as_str(),
            tx_rate,
            token_ttl_s: TOKEN_TTL.as_secs(),
        })
    }

    /// Consume a token: `Ok` only for a live, unused grant (removed on use —
    /// one connection per token).
    fn take_grant(&self, token: &str) -> std::result::Result<Grant, ()> {
        let mut grants = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        match grants.get(token).map(|g| g.expires > Instant::now()) {
            Some(true) => Ok(grants.remove(token).expect("just checked")),
            _ => Err(()),
        }
    }

    /// Kill an active stream by id (token).
    pub(crate) fn close_stream(&self, stream_id: &str) -> Result<()> {
        let kill = self
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(stream_id)
            .cloned();
        match kill {
            Some(k) => {
                let _ = k.send(());
                Ok(())
            }
            None => anyhow::bail!("no active stream with id `{stream_id}`"),
        }
    }
}

/// Entry point from the hub's accept loop: complete the WS handshake (validating
/// the token path) and spawn the connection task. Rejects unknown/expired
/// tokens with HTTP 404 — the token IS the authorization.
pub(crate) fn accept(state: Arc<BridgeState>, hub: Arc<Hub>, tcp: TcpStream) {
    tokio::spawn(async move {
        // The handshake callback validates (and consumes) the token and hands
        // the grant's data out through this slot.
        let mut granted: Option<(String, Grant)> = None;
        // The callback's error type is fixed by tungstenite's trait bound (a
        // full HTTP response) — too large for clippy's taste, not ours to change.
        #[allow(clippy::result_large_err)]
        let check = |req: &Request,
                     res: Response|
         -> std::result::Result<
            Response,
            tokio_tungstenite::tungstenite::http::Response<Option<String>>,
        > {
            let path = req.uri().path();
            if let Some(token) = path.strip_prefix(TOKEN_PATH_PREFIX) {
                if let Ok(grant) = state.take_grant(token) {
                    granted = Some((token.to_string(), grant));
                    return Ok(res);
                }
            }
            Err(tokio_tungstenite::tungstenite::http::Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Some("no such stream token\n".to_string()))
                .expect("static 404 response"))
        };
        // A rejected handshake (bad token) already sent its 404; nothing
        // to clean up.
        if let Ok(ws) = tokio_tungstenite::accept_hdr_async(tcp, check).await {
            if let Some((token, grant)) = granted {
                handle_conn(state, hub, token, grant, ws).await;
            }
        }
    });
}

/// Client→server control frames.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientControl {
    Ping,
    /// Barge-in: drop queued TX audio, re-arm the stream.
    FlushTx,
}

async fn handle_conn(
    state: Arc<BridgeState>,
    hub: Arc<Hub>,
    token: String,
    grant: Grant,
    ws: WebSocketStream<TcpStream>,
) {
    let agent = match hub.get(&grant.agent).await {
        Ok(a) => a,
        Err(_) => return,
    };

    let (mut sink, mut source) = ws.split();

    // Register the kill switch (stream_close).
    let (kill_tx, mut kill_rx) = mpsc::unbounded_channel();
    state
        .active
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(token.clone(), kill_tx);

    // Arm TX FIRST — before any inbound audio can be forwarded, the arming
    // command must be queued in the single writer (worker-side ordering).
    let tx_chan = if grant.mode != StreamMode::Rx {
        let c = agent.tx_channel().await;
        let _ = c
            .send(TxMsg::Start {
                rate: grant.tx_rate,
            })
            .await;
        Some(c)
    } else {
        None
    };

    let mut rx_sub = if grant.mode != StreamMode::Tx {
        Some(agent.rx_frames().await)
    } else {
        None
    };
    let mut ev_sub = agent.subscribe_events();
    let mut announced_rate: Option<u32> = None;

    loop {
        tokio::select! {
            // stream_close / hub teardown
            _ = kill_rx.recv() => break,

            // Inbound from the client.
            msg = source.next() => {
                match msg {
                    Some(Ok(Message::Binary(data))) => {
                        if let Some(tx) = tx_chan.as_ref() {
                            let _ = tx.send(TxMsg::Audio(pcm_from_bytes(&data))).await;
                        }
                        // rx-only streams have no writer: audio is ignored.
                    }
                    Some(Ok(Message::Text(text))) => {
                        let reply = match serde_json::from_str::<ClientControl>(&text) {
                            Ok(ClientControl::Ping) => json!({"type": "pong"}),
                            Ok(ClientControl::FlushTx) => {
                                // Re-arm = flush: the worker drops whatever
                                // was still queued (see start_audio_stream).
                                if let Some(tx) = tx_chan.as_ref() {
                                    let _ = tx.send(TxMsg::Start { rate: grant.tx_rate }).await;
                                }
                                json!({"type": "tx_flushed"})
                            }
                            Err(e) => json!({"type": "error", "message": format!("bad control frame: {e}")}),
                        };
                        let _ = sink.send(Message::Text(reply.to_string().into())).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    // Protocol pings are answered by tungstenite itself.
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }

            // Received audio → client (binary), rate announced first.
            frame = async {
                match rx_sub.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                match frame {
                    Ok(f) => {
                        if announced_rate != Some(f.rate) {
                            announced_rate = Some(f.rate);
                            let _ = sink.send(Message::Text(
                                json!({"type": "rx_started", "rate": f.rate}).to_string().into(),
                            )).await;
                        }
                        if sink.send(Message::Binary(pcm_to_bytes(&f.samples).into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(dropped)) => {
                        let _ = sink.send(Message::Text(
                            json!({"type": "rx_lagged", "dropped": dropped}).to_string().into(),
                        )).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // Worker gone; nothing more will arrive.
                        let _ = sink.send(Message::Text(
                            json!({"type": "error", "message": "agent audio stream ended"}).to_string().into(),
                        )).await;
                        rx_sub = None;
                    }
                }
            }

            // Agent events → client (text), same shape as wait_event results.
            ev = ev_sub.recv() => {
                if let Ok(event) = ev {
                    let _ = sink.send(Message::Text(event_text(&event).to_string().into())).await;
                }
            }
        }
    }

    state
        .active
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&token);
    let _ = sink.close().await;
}

/// An agent event as a WS text frame: the `wait_event` JSON with `"event"`
/// renamed to `"type"` (one uniform text-frame tag for consumers).
fn event_text(e: &ringo_core::event::AppEvent) -> serde_json::Value {
    let mut v = crate::server::event_json(e);
    if let Some(obj) = v.as_object_mut() {
        if let Some(name) = obj.remove("event") {
            obj.insert("type".to_string(), name);
        }
    }
    v
}

/// Little-endian s16 bytes (mono) → samples.
fn pcm_from_bytes(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

/// Samples → little-endian s16 bytes (mono).
fn pcm_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut v = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}
