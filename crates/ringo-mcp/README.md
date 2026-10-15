# ringo-mcp

An [MCP](https://modelcontextprotocol.io) server over stdio that exposes ringo
SIP agents to LLM agents for telephony: place calls, accept incoming calls, send
DTMF, transfer, observe call events and media quality.

## How it works

- Each `[[agent]]` in the config gets its own `ringo-agent` **worker process**
  (one SIP user agent each, with its own port and registration), driven over the
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

Optional flags: `--enabled-tools <LIST>` / `--disable <GROUP|TOOL>` trim the
tool surface (groups: discovery, call-control, audio, headers, events,
recording, lifecycle, trace — or individual tool names);
`--dial-deny <REGEX>` / `--dial-allow <REGEX>` (repeatable, global) restrict
what `dial`/`transfer` may place a call to — deny always wins, e.g.
`--dial-deny '^00' --dial-deny '^0900'`; `--version` prints the version.

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
# hold_other_calls (default false), record_audio (record every call by default)
```

Multiple agents = multiple UAs, one process each. Each registers its own AOR;
calls between them (or to/from the outside) are routed by your SIP
infrastructure. `catchall` defaults to `true` (safe here because each worker is
the only UA in its process — see the ringo-core `Account` docs).

Agents can also take their account from ringo-phone profiles — per agent or
in bulk — so SIP credentials and custom headers live in one place:

```toml
[[agent]]
profile = "[prod] Office"      # username/domain/credentials/headers from the profile
name = "prod-office"            # optional — defaults to the profile name's slug
dtmf_mode = "info"              # explicit fields override the profile

[ringo]
# Or in bulk: every ringo-phone profile whose name matches a regex becomes an
# agent (unanchored, case-sensitive — escape `[` and `]`).
profiles = ['^\[test\]']
```

`list_agents`/`agent_status` show such an agent's `profile` origin and its
`description`/`metadata` (e.g. `metadata.number`, so the LLM knows which
number reaches which agent). Patterns
select accounts, not just names — a broad regex exposes LIVE trunks to the LLM
too; the configuration docs have the rules and pitfalls.

## Tools

| Tool | Description |
| --- | --- |
| `list_agents` | All configured agents: name, AOR, running, registration, calls |
| `agent_status` | One agent: registration, calls, media stats, received DTMF (starts the agent if not yet running) |
| `dial` | Outgoing call (`agent`, `target`: URI, `user@host`, or bare extension; optional `record: true` records the call); optional `wait_established` blocks until the outcome (established / failed with reason / still-ringing with call_id) |
| `accept` / `hangup` / `hangup_all` | Answer (optional `record: true`) / end calls |
| `hold` / `resume` / `mute` | In-call control. `agent_status` reflects local holds (`"held"` phase); peer holds arrive as `call_hold`/`call_resume` events |
| `send_dtmf` | One digit (`0-9`, `*`, `#`, `A-F`) |
| `transfer` | Blind transfer of the current call |
| `play` | What the agent transmits: `"silence"`, `"ausine,425"`, `"aufile,<wav>"` (resets to silence when the last call ends) |
| `wait_event` | Block for the agent's next event (JSON), up to 120 s; optional event filter and `after_id` log cursor (no lost events across tool roundtrips) |
| `agent_events` | Read the agent's event log with id cursors (catch up after looking away) |
| `agent_stop` | Stop an agent's worker (deregister + exit; restarts on next use) |
| `call_headers` | SIP headers of received INVITEs, per Call-ID or all (newest first) |
| `add_header` / `rm_header` | Custom headers on outgoing INVITEs (runtime; `${uuid}` renders once, immediately) |
| `record_start` / `record_stop` | Record an agent's calls (sent + received; `record_start` starts fresh, `record_stop` keeps the audio for `save_audio`) |
| `save_audio` | Write the recorded audio to WAVs (paths, durations, truncation flag) |
| `sip_trace` | Recent SIP messages of an agent (plaintext, also TLS), filterable by Call-ID |

Call control is fire-and-forget: observe progress with `wait_event`
(`call_ringing` → `call_established` → `call_closed`) or by polling
`agent_status`.

### Events

`wait_event` speaks these event names:
`registering`, `register_ok`, `register_failed`, `unregistered`,
`call_incoming`, `call_outgoing`, `call_ringing`, `call_established`,
`call_closed` (with `reason`/`error`), `call_deflected`, `call_hold` /
`call_resume` (the **peer** held or resumed), `call_transfer_failed`,
`voicemail_status`, `response`, `backend_connect_failed`. `wait_event` takes an
optional `event` filter — a name or an array, e.g.
`["call_established", "call_closed"]` to await a call's outcome either way.

## Logging

Workers are silent unless `RINGO_AGENT_LOG` is set in the server's environment
(`-` for stderr, a path for a per-agent file); it is inherited by the worker
processes. SIP traces are on by default (text, in a private `$XDG_RUNTIME_DIR/ringo-mcp-*`
dir, removed on exit) and readable via `sip_trace`; set `RINGO_AGENT_SIPTRACE` to
write them elsewhere (a `.pcap` path for sngrep/Wireshark, `-` for stderr) —
`sip_trace` then only works for a text file.
