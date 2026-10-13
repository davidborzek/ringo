# Examples

Recipes for the flows an LLM agent typically drives. All examples show the
tool calls (name + arguments), not the JSON-RPC envelope.

## Outbound call, then DTMF

The classic IVR interaction: dial, wait for the call to establish, navigate.

```jsonc
dial          { "agent": "alice", "target": "0800123456" }
wait_event    { "agent": "alice" }            // → call_ringing
wait_event    { "agent": "alice" }             // → call_established
send_dtmf     { "agent": "alice", "digit": "1" }
send_dtmf     { "agent": "alice", "digit": "#" }
hangup        { "agent": "alice" }
```

If the number is busy, `wait_event` returns `call_closed` with the reason
instead — no need to poll for failure separately.

## Inbound call

Agents register automatically; just sit on `wait_event` (e.g. with a long
`timeout_ms`) and accept when something comes in.

```jsonc
wait_event    { "agent": "support", "timeout_ms": 120000 }
// → { "event": "call_incoming", "from": "sip:1002@pbx.example.com", … }
call_headers  { "agent": "support", "call_id": "…" }   // who/what is calling
accept        { "agent": "support" }
wait_event    { "agent": "support" }            // → call_established
```

`call_headers` is how the agent inspects the INVITE — caller identity beyond
the From-URI, correlation IDs, provider-specific routing headers.

## A call between two configured agents

Two agents in one config are two UAs, each in its own process — they can call
each other via the SIP infrastructure:

```jsonc
dial          { "agent": "alice", "target": "1002" }   // bare extension
wait_event    { "agent": "bob" }             // → call_incoming on the callee
accept        { "agent": "bob" }
wait_event    { "agent": "alice" }            // → call_established on the caller
// both sides are now up; media stats are per-agent:
agent_status  { "agent": "bob" }              // → media_stats.mos, jitter, …
hangup        { "agent": "alice" }            // either side can end it
```

## Playing audio into a call

Headless agents are silent until told otherwise. Feed a tone or a WAV, then go
back to silence:

```jsonc
play          { "agent": "alice", "spec": "aufile,/opt/prompts/hello.wav" }
play          { "agent": "alice", "spec": "ausine,425" }   // a 425 Hz tone
play          { "agent": "alice", "spec": "silence" }
```

`play` is call-scoped: when the agent's last call ends, it resets to silence
automatically — the next call starts quiet.

## Transfer

Hand a call off (blind transfer of the current call):

```jsonc
dial          { "agent": "alice", "target": "1002" }
wait_event    { "agent": "alice" }            // → call_established
transfer      { "agent": "alice", "target": "1003" }
wait_event    { "agent": "alice" }            // → call_closed ("Call transfered")
```

## Correlation headers, end to end

Declare a per-call correlation id in the
[config](configuration.md#custom-headers):

```toml
[[agent]]
name = "alice"
# … account fields …
custom_headers = [["X-Session-Tag", "session-${uuid}"]]
```

Every outgoing INVITE from `alice` now carries a *fresh* value. On the
receiving side (any agent, or your own infrastructure) it's readable back
with `call_headers`:

```jsonc
dial          { "agent": "alice", "target": "0800123456" }
call_headers  { "agent": "support" }
// → [ "X-Session-Tag", "session-3f2c1a9e-8d47-4b60-b2f4-9c1d5a7e6a10" ]
```

For one-off headers (fixed value, set at runtime), use `add_header` instead:

```jsonc
add_header    { "agent": "alice", "name": "X-Session-Tag", "value": "demo-42" }
```

## Waiting for several calls

`wait_event` returns *one* event per call. When several concurrent calls are
in flight, the agent typically loops: `wait_event` → look at
`agent_status` (`calls[]` with phases) → act on the relevant `call_id`.

```jsonc
wait_event    { "agent": "support", "timeout_ms": 60000 }  // → call_incoming A
wait_event    { "agent": "support", "timeout_ms": 60000 }  // → call_incoming B
agent_status  { "agent": "support" }     // both listed as ringing
accept        { "agent": "support" }      // accept the current line
```
