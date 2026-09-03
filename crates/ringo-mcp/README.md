# ringo-mcp

An [MCP](https://modelcontextprotocol.io) server over stdio that exposes ringo
SIP agents to LLM agents for telephony: place calls, accept incoming calls, send
DTMF, transfer, observe call events and media quality.

## How it works

- Each `[[agent]]` in the config gets its own `ringo-agent` **worker process**
  (one baresip UA each, with its own SIP port and registration), driven over the
  framed stdio protocol — the same architecture ringo-flow uses.
- **Agents start lazily**: the config is validated at startup (no processes),
  and a worker is spawned by the first tool call that touches an agent
  (single-flight — concurrent calls share one spawn). A worker that died is
  respawned on the next tool call. `list_agents` shows configured-but-not-
  running agents with `"running": false`.
- The binary re-invokes itself as `ringo-mcp agent` for those workers, so server
  and workers always share one build.
- The MCP server speaks JSON-RPC on stdin/stdout; worker diagnostics go to
  stderr (MCP clients surface them as logs).

## Install into an MCP client

```json
{
  "mcpServers": {
    "ringo": {
      "command": "ringo-mcp",
      "args": ["--config", "/path/to/config.toml"]
    }
  }
}
```

Without `--config`, the path comes from `$RINGO_MCP_CONFIG` or defaults to
`~/.config/ringo-mcp/config.toml`.

## Config

```toml
[[agent]]
name = "alice"                    # unique label used by every tool
username = "1001"
domain = "pbx.example.com"
password = "secret"
# password_file = "~/secrets/alice"     # or: password_cmd = "pass show sip/alice"
# display_name, auth_user, transport, outbound, stun_server, media_enc,
# regint, mwi, dtmf_mode, catchall, audio_codecs — see the ringo-core Account.
# Custom headers on outgoing INVITEs (array of pairs, or a table):
# custom_headers = [["X-Session-Tag", "session-${uuid}"]]
# `${uuid}` re-renders on every outgoing call; `$$` is a literal `$`.

[[agent]]
name = "bob"
username = "1002"
domain = "pbx.example.com"
password = "other-secret"
dtmf_mode = "info"   # SIP-INFO DTMF — reliable when the audio source is idle

[backend]
audio_driver = "aubridge"   # headless default (no sound hardware needed)
# max_calls, local_timeout_s, sip_cafile, sip_capath, user_agent,
# hold_other_calls (default false), record_audio (for save_audio)
```

Multiple agents = multiple UAs, one process each. Each registers its own AOR;
calls between them (or to/from the outside) are routed by your SIP
infrastructure. `catchall` defaults to `true` (safe here because each worker is
the only UA in its process — see the ringo-core `Account` docs).

## Tools

| Tool | Description |
| --- | --- |
| `list_agents` | All configured agents: name, AOR, running, registration, calls |
| `agent_status` | One agent: registration, calls, media stats, received DTMF (starts the agent if not yet running) |
| `dial` | Outgoing call (`agent`, `target`: URI, `user@host`, or bare extension) |
| `accept` / `hangup` / `hangup_all` | Answer / end calls |
| `hold` / `resume` / `mute` | In-call control |
| `send_dtmf` | One digit (`0-9`, `*`, `#`, `A-F`) |
| `transfer` | Blind transfer of the current call |
| `play` | What the agent transmits: `"silence"`, `"ausine,425"`, `"aufile,<wav>"` (resets to silence when the last call ends) |
| `wait_event` | Block for the agent's next event (JSON), up to 120 s; optionally filtered by event name(s) |
| `agent_stop` | Stop an agent's worker (deregister + exit; restarts on next use) |
| `call_headers` | SIP headers of received INVITEs, per Call-ID or all (newest first) |
| `add_header` / `rm_header` | Custom headers on outgoing INVITEs (runtime; `${uuid}` renders once, immediately) |
| `stream_open` / `stream_close` | Live audio over WebSocket: `ws://` URL carrying raw mono PCM + pushed events (see below) |
| `save_audio` | Write the current call's audio to WAVs (needs `record_audio`) |

Call control is fire-and-forget: observe progress with `wait_event`
(`call_ringing` → `call_established` → `call_closed`) or by polling
`agent_status`.

## Live audio (WebSocket)

MCP can't stream, so `stream_open` returns a `ws://127.0.0.1:<ephemeral>/s/<token>`
URL: binary frames are raw mono s16le PCM (both directions; the RX rate is
announced via an `rx_started` text frame, TX must be sent at the negotiated
`tx_rate`), text frames are control JSON — `ping`/`pong`, `flush_tx`
(barge-in: drops queued audio, re-arms the stream) and the agent's call events
pushed live. One connection per token, 300 s TTL, loopback only (remote goes
through a reverse proxy). This is the transport for STT/TTS pipelines.

## Logging

Workers are silent unless `RINGO_AGENT_LOG` is set in the server's environment
(`-` for stderr, a path for a per-agent file); same for `RINGO_AGENT_SIPTRACE`.
Both are inherited by the worker processes.
