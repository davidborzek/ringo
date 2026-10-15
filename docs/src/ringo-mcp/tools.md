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
      "worker_dead": null, "calls": [],
      "profile": "[test] Alice",
      "description": "Front desk, answers incoming calls",
      "metadata": { "number": "+492111234567" } } ] }
```

`running: false` = configured but not started since server start.
`registered: null` = agent is starting up (or not running); query
`agent_status` for live values. `profile` is present only for agents built
from a ringo-phone profile — the profile's name. `description` and `metadata`
are present when configured: notes for the LLM, e.g. `metadata.number` = the
number to dial to reach that agent (see
[configuration](configuration.md#telling-the-llm-how-to-reach-agents)).

### `agent_status`

One agent's full state — registration, live calls, media quality, received
DTMF, the last close reason. Poll this after `dial`/`accept` to check progress.

```jsonc
{ "name": "alice", "aor": "sip:1001@pbx.example.com",
  "profile": "[test] Alice", "metadata": { "number": "+492111234567" },
  "registered": true, "reg_error": null, "worker_dead": false,
  "calls": [ { "id": "a1b2…", "phase": "established",
               "peer": "sip:1002@pbx.example.com", "peer_name": "Bob" } ],
  "call_count": 1,
  "media_stats": { "rtt_ms": 21.5, "jitter_ms": 2.1, "rx_lost": 0,
                   "packet_loss_pct": 0.0, "mos": 4.4 },
  "received_dtmf": "12",
  "last_call_reason": null, "last_call_error": false }
```

`profile` is present only for agents built from a ringo-phone profile — the
profile's name.

## Call control

Call control is fire-and-forget: the tools return as soon as the command is
sent. Observe outcomes with `wait_event` (preferred) or by polling
`agent_status` — **or** make `dial` itself block with
`wait_established: true` (and an optional `timeout_ms`, default 60000,
max 120000). The dial then returns the outcome directly:

```jsonc
{"established": true, "call_id": "…"}                          // up
{"established": false, "call_id": "…", "reason": "486 Busy…"}   // failed
{"established": false, "call_id": "…", "state": "ringing", "waited_ms": 60000}
{"established": false, "error": "dial produced no call within 10 s …"}
```

A timeout does **not** cancel the call — it keeps ringing, and the reply
carries its `call_id` (hang up or keep waiting via `wait_event`). A target
the worker rejects locally produces no call at all; the 10 s confirm
deadline surfaces that instead of blocking the full budget. Concurrent dials
on one agent attribute to the first `call_outgoing` — dial one at a time
when using `wait_established`.

| Tool | Args | Effect |
| ---- | ---- | ------ |
| `agent_stop` | `agent` | Stop the agent's worker (deregister + exit). Idempotent; the agent starts again on its next use. |
| `dial` | `agent`, `target`, `record?` | Place an outgoing call. `target` is a full URI, `user@host`, or a bare number/extension (resolved to `sip:<target>@<agent domain>`); `tel:`/`callto:` and number separators are stripped. Subject to the `--dial-allow`/`--dial-deny` policy. Re-renders dynamic header templates (fresh `${uuid}`). `record: true` records this call (see [Recording](#recording)). |
| `accept` | `agent`, `record?` | Accept the currently ringing (incoming) call. `record: true` records this call (see [Recording](#recording)). |
| `hangup` | `agent` | Hang up the current call. |
| `hangup_all` | `agent` | Hang up all calls. |
| `hold` / `resume` | `agent` | Hold / resume. |
| `mute` | `agent` | Mute the agent's outgoing audio. |
| `send_dtmf` | `agent`, `digit` | One digit: `0-9`, `*`, `#`, `A-F`. |
| `transfer` | `agent`, `target` | Blind-transfer the current call (same resolution and dial policy as `dial`). |
| `play` | `agent`, `spec` | What the agent transmits: `"silence"`, `"ausine,<freq>"` (e.g. `ausine,425`) or `"aufile,<path>"` (mono WAV). Call-scoped: resets to silence when the agent's last call ends, so the next call won't replay it. |
| `hold` / `resume` | `agent` | Hold / resume. `agent_status` reflects the local hold optimistically (the `held` phase); peer-initiated hold arrives as `call_hold`/`call_resume` events. |

## Events

### `wait_event`

Blocks until the agent's *next* event, or a timeout. `timeout_ms` defaults to
30000, capped at 120000. An optional `event` filter waits for a specific event
(or any of several) and skips the rest — other waiters and the state fold still
see them. An optional `after_id` cursor switches to log semantics (see
[The event log](#the-event-log)): events that fired since that id are delivered
from the log, oldest-first, and the reply carries the matched event's `id` as
the next cursor:

```jsonc
{ "agent": "alice", "event": ["call_established", "call_closed"], "timeout_ms": 30000 }
```

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
`call_deflected`, `call_hold` / `call_resume` (the **peer** held or
resumed the call), `call_transfer_failed`, `voicemail_status`,
`response`, `backend_connect_failed`. Backend-internal events (SDP
negotiation, RTP/RTCP mechanics, lifecycle internals) never surface —
current media stats are one `agent_status` away. A timeout returns
`{ "timeout": true }`.

Without `after_id`, subscribing happens at call time — past events are not
replayed (use the event log below instead; `agent_status` has the state).

### The event log

Every agent keeps a bounded log of its events with monotonically increasing
ids. Read it with `agent_events`:

```jsonc
{ "agent": "alice", "after_id": 42 }
// → { "events": [{"id": 43, "event": "call_established", …}, …],
//     "count": 1, "oldest_id": 12, "newest_id": 43, "truncated": false }
```

- `after_id` (default 0) returns only newer events, oldest first, at most 100
  per call — page with the last returned id.
- `truncated: true` means the cursor fell off the ring (the last 256 events
  are kept): older events were lost.

The log closes the roundtrip gap: take the cursor (`newest_id`), act
(`accept`, `dial`, …), then `wait_event` with `after_id` — an event that fired
*during* the roundtrip is delivered, not lost.

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

## Recording

Recording captures a call's sent + received audio in-process (headless
agents capture both sides; hold/resume does not cut a recording). Turn it on
per call — `record_start` before or at the call start, or `record: true` on
`dial`/`accept` — or for every call with `record_audio = true` in `[backend]`.
One recording per agent; parallel calls mix into it.

### `record_start` / `record_stop`

```jsonc
{ "agent": "alice" }
// → "recording started for `alice` (save with `save_audio`; …)"
```

`record_start` arms recording now and starts a fresh recording (a previous
recording is cleared); arm before or at the start of a call to capture all of
it. `record_stop` disarms; the recorded audio is kept for `save_audio` until
the next recording or call starts. A per-call recording (`record_start` /
`record: true`) is disarmed automatically when the agent's last call ends —
the audio stays saveable.

### `save_audio`

```jsonc
{ "agent": "alice", "prefix": "call" }
// → { "files": [
//       { "path": "call-sent.wav", "kind": "sent",
//         "duration_ms": 42000, "truncated": false },
//       { "path": "call-recv.wav", "kind": "recv",
//         "duration_ms": 42000, "truncated": false } ] }
```

Writes the recorded sent + received audio as `<prefix>-sent.wav` /
`<prefix>-recv.wav`, each with its duration. `truncated: true` means the call
was longer than the 600 s retention cap — the WAV holds only the newest audio.
Works during and after the call, until the next recording or call starts.
Errors clearly when recording was never on for the agent (call `record_start`,
pass `record: true`, or set `record_audio = true` in `[backend]`), and when
recording is on but nothing has been captured yet.

## Trace

### `sip_trace`

```jsonc
{ "agent": "alice", "call_id": "a1b2…", "after_id": 3920, "limit": 20 }
// → { "messages": [{ "id": 4711, "ts": "10:00:00.050", "direction": "rx",
//       "transport": "UDP", "first_line": "SIP/2.0 488 Not Acceptable Here",
//       "call_id": "a1b2…", "text": "SIP/2.0 488 …" }],
//     "count": 1, "skipped": 0, "newest_id": 4711, "truncated": false }
```

The agent's SIP messages, sent and received, in plaintext (also for TLS: the
trace is taken before encryption / after decryption) — to see *why* a call
failed: the exact final response, the SDP, the headers.

- `call_id` (optional) keeps one call's dialog; take it from a `call_*` event
  or `agent_status`.
- `after_id` (optional) returns only newer messages; `newest_id` is the next
  cursor. Ids are opaque, increasing numbers (positions in the trace).
- Only the newest 2 MiB of the trace are read; `truncated: true` means older
  messages exist beyond that (or behind your cursor).
- `limit` (default 20, max 100) keeps the newest matching messages; `skipped`
  counts the older ones left out. A single message is cut at 8 KiB.
- The trace belongs to the current worker: after `agent_stop` or a crash the
  next worker starts a fresh one, and ids start over.
- Traces include the `Authorization` headers (digest responses, never the
  password). Disable the tool with `--disable trace` if the client must not
  see them.
