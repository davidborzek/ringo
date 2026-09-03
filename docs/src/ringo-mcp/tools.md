# Tool reference

Every tool takes an `agent` name (from the config file) unless noted. The first
tool call for an agent starts its worker process; a dead worker is respawned
the same way. Results are JSON (shown here pretty-printed; on the wire they're
one JSON string).

## Discovery

### `list_agents`

All configured agents. Spawns nothing.

```jsonc
{ "agents": [
    { "name": "alice", "aor": "sip:1001@pbx.example.com",
      "running": false, "registered": null, "reg_error": null,
      "worker_dead": null, "calls": [] } ] }
```

`running: false` = configured but not started since server start.
`registered: null` = agent is starting up (or not running); query
`agent_status` for live values.

### `agent_status`

One agent's full state — registration, live calls, media quality, received
DTMF, the last close reason. Poll this after `dial`/`accept` to check progress.

```jsonc
{ "name": "alice", "aor": "sip:1001@pbx.example.com",
  "registered": true, "reg_error": null, "worker_dead": false,
  "calls": [ { "id": "a1b2…", "phase": "established",
               "peer": "sip:1002@pbx.example.com", "peer_name": "Bob" } ],
  "call_count": 1,
  "media_stats": { "rtt_ms": 21.5, "jitter_ms": 2.1, "rx_lost": 0,
                   "packet_loss_pct": 0.0, "mos": 4.4 },
  "received_dtmf": "12",
  "last_call_reason": null, "last_call_error": false }
```

## Call control

Call control is fire-and-forget: the tools return as soon as the command is
sent. Observe outcomes with `wait_event` (preferred) or by polling
`agent_status`.

| Tool | Args | Effect |
| ---- | ---- | ------ |
| `dial` | `agent`, `target` | Place an outgoing call. `target` is a full URI, `user@host`, or a bare number/extension (resolved to `sip:<target>@<agent domain>`). Re-renders dynamic header templates (fresh `${uuid}`). |
| `accept` | `agent` | Accept the currently ringing (incoming) call. |
| `hangup` | `agent` | Hang up the current call. |
| `hangup_all` | `agent` | Hang up all calls. |
| `hold` / `resume` | `agent` | Hold / resume. |
| `mute` | `agent` | Mute the agent's outgoing audio. |
| `send_dtmf` | `agent`, `digit` | One digit: `0-9`, `*`, `#`, `A-F`. |
| `transfer` | `agent`, `target` | Blind-transfer the current call (same target resolution as `dial`). |
| `play` | `agent`, `spec` | What the agent transmits: `"silence"`, `"ausine,<freq>"` (e.g. `ausine,425`) or `"aufile,<path>"` (mono WAV). Call-scoped: resets to silence when the agent's last call ends, so the next call won't replay it. |

## Events

### `wait_event`

Blocks until the agent's *next* event, or a timeout. `timeout_ms` defaults to
30000, capped at 120000.

```jsonc
{ "agent": "alice", "timeout_ms": 30000 }
```

```jsonc
{ "event": "call_incoming", "call_id": "a1b2…",
  "from": "sip:1002@pbx.example.com", "display_name": "Bob" }
```

Event types: `registering`, `register_ok`, `register_failed`,
`unregistered`, `call_incoming`, `call_outgoing`, `call_ringing`,
`call_established`, `call_closed` (with `reason` / `error`),
`call_deflected`, `voicemail_status`, `response`, `unknown`,
`backend_connect_failed`. A timeout returns `{ "timeout": true }`.

Subscribing happens at call time — past events are not replayed (poll
`agent_status` for state). Events can also arrive *between* tool calls; if an
expected event never comes, call `wait_event` again — anything that happened
in between is reflected in `agent_status` regardless.

## Headers

### `call_headers`

SIP headers of received INVITEs, as `[[name, value], …]` pairs — order and
duplicates preserved. Filter by `call_id` or omit it for all known calls
(newest first). Headers persist after the call closes (capped at the 128 most
recent calls). They lag `call_incoming` by ~150 ms — retry once if empty
right after the event.

```jsonc
{ "agent": "alice", "call_id": "a1b2…" }
```

```jsonc
{ "call_id": "a1b2…", "headers": [
    [ "From", "\"Bob\" <sip:1002@pbx.example.com>;tag=…" ],
    [ "X-Session-Tag", "session-3f2c1a9e-8d47-4b60-b2f4-9c1d5a7e6a10" ],
    [ "Record-Route", "<sip:proxy.example.com;lr>" ] ] }
```

### `add_header` / `rm_header`

Custom headers on the agent's **outgoing** INVITEs, at runtime.

```jsonc
{ "agent": "alice", "name": "X-Session-Tag", "value": "session-${uuid}" }
```

- `add_header`: templates render **once, immediately** — `${uuid}` becomes a
  fixed value for this and future calls. For a fresh uuid per call, declare it
  in the [config](configuration.md#custom-headers) instead. Adding replaces
  any existing header of that name.
- `rm_header`: removes *all* headers with that name.
- Runtime headers persist until the worker restarts (config-declared headers
  are re-applied on respawn).

## Live audio (WebSocket)

MCP can't stream, so raw audio gets its own channel: a small WebSocket
server (loopback only) next to the stdio control plane.

### `stream_open`

```jsonc
{ "agent": "alice", "mode": "duplex", "tx_rate": 16000 }
```

Mints a one-shot token (valid 300 s) and returns the URL:

```jsonc
{ "url": "ws://127.0.0.1:53712/s/<token>", "stream_id": "<token>",
  "mode": "duplex", "tx_rate": 16000, "token_ttl_s": 300,
  "protocol": { … } }
```

`mode` is `"rx"` (call audio → you, e.g. for STT), `"tx"` (you → call, e.g.
TTS) or `"duplex"`. `tx_rate` is the rate you must send at (default 16000);
the RX rate arrives before the first audio frame.

### The wire protocol

- **Binary frames** = raw mono s16le PCM. Server→client: the agent's received
  audio. Client→server: audio into the call.
- **Text frames** = control JSON, tagged with `"type"`:

| Direction | Type | Meaning |
| --------- | ---- | ------- |
| ← server | `rx_started` | Announces the RX rate (before the first binary frame). |
| ← server | `rx_lagged` | You fell behind; `dropped` frames were skipped. |
| ← server | `tx_flushed` | A `flush_tx` completed (queue dropped, stream re-armed). |
| ← server | `pong` | Reply to `ping`. |
| ← server | `error` | Protocol/lifecycle error message. |
| ← server | `call_incoming`, `call_established`, `call_closed`, … | The agent's events, pushed live (same fields as `wait_event`). |
| → server | `ping` | Keepalive. |
| → server | `flush_tx` | Barge-in: drop queued TX audio and re-arm the stream. |

This is the transport for STT/TTS pipelines and live listening — ringo-mcp
moves dumb PCM, the speech processing stays with the consumer:

```
stream_open(rx) → STT daemon reads PCM from the socket → transcript
stream_open(tx) → TTS output written into the socket → played into the call
```

### `stream_close`

```jsonc
{ "stream_id": "<token from stream_open>" }
```

Kills the connection (and frees the stream slot).

## Recording

### `save_audio`

```jsonc
{ "agent": "alice", "prefix": "call" }
// → { "files": ["call-a1b2…-01.wav"] }
```

Writes the current call's sent + received audio as WAVs. Requires
`record_audio = true` in `[backend]`.
