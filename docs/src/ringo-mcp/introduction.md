<h1 class="lockup">
  <img class="lockup-mark" src="../logo.svg" alt="" />
  <span class="lockup-text"><span class="lockup-name">ringo</span><span class="lockup-sub">mcp</span></span>
</h1>

**ringo-mcp** is an [MCP](https://modelcontextprotocol.io) server that gives
LLM agents a telephone: SIP agents configured in a TOML file, driven from any
MCP client (Claude Code, Cursor, Pi, …) over stdio. Place and answer calls,
send DTMF, transfer, inspect INVITE headers and media quality — natural
language in, SIP out.

```
you: "Call the support line, wait for the IVR menu and press 2"
 └─ MCP client → ringo-mcp ─┐
                            ├─ dial("support", "0800123456")
                            ├─ wait_event → call_established
                            ├─ send_dtmf("2")
                            └─ …all over SIP, via baresip
```

## Highlights

- **Telephony without glue code** — one config file maps SIP accounts to named
  agents; the LLM addresses them by name.
- **Multiple UAs** — each agent is its own worker process (own baresip UA,
  own SIP port and registration), so agents can call each other or the outside
  world. They start lazily, on first use.
- **Event-driven, not guesswork** — call control is fire-and-forget; outcomes
  arrive as events (`call_incoming`, `call_established`, `call_closed`) via the
  blocking `wait_event` tool, or by polling `agent_status`.
- **Header round-trip** — declare custom headers with per-call templates
  (`${uuid}`), add them at runtime, and read the headers of received INVITEs
  back out.
- **Headless by default** — no audio hardware needed; agents transmit silence,
  tones or WAV files via `play`.

## Architecture

`ringo-mcp` is a thin MCP layer over [ringo-agent]'s process protocol: one
worker process per configured agent, speaking a framed stdio protocol. The
binary re-invokes itself (`ringo-mcp agent`) for those workers, so server and
workers always share one build. The same one-process-per-UA design powers
[ringo-flow]'s scenario runner.

```
MCP client ──stdio(JSON-RPC)── ringo-mcp ──framed stdio── agent worker (baresip UA)
                                             ├─ agent worker (baresip UA)
                                             └─ …
```

[ringo-agent]: https://docs.rs/ringo-agent
[ringo-flow]: https://davidborzek.github.io/ringo/ringo-flow/introduction.html

## Next steps

- [Getting started](getting-started.md) — install, configure, wire into your
  MCP client, first call.
- [Configuration](configuration.md) — the config file reference.
- [Tool reference](tools.md) — all tools with their JSON payloads.
- [Examples](examples.md) — recipes for the common call flows.

The Rust library API is on [docs.rs](https://docs.rs/ringo-mcp).
