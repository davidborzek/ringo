# Getting started

## Install

baresip is built in and statically linked — no separate `baresip` install
needed.

**Pre-built binaries** for Linux and macOS (x86\_64 + arm64) are on the
[releases page](https://github.com/davidborzek/ringo/releases) — download,
extract and put `ringo-mcp` on your `$PATH`.

**From crates.io:**

```sh
cargo install ringo-mcp
```

**From GitHub (no clone needed):**

```sh
cargo install --git https://github.com/davidborzek/ringo ringo-mcp
```

**From a workspace checkout** (no install):

```sh
cargo run -p ringo-mcp -- serve
```

## Configure your agents

Create `~/.config/ringo-mcp/config.toml` (or pass `--config <path>`, or set
`RINGO_MCP_CONFIG`). One `[[agent]]` per SIP identity:

```toml
[[agent]]
name = "support"                 # the name every tool addresses
username = "1001"
domain = "pbx.example.com"
password = "secret"
# password_file = "~/secrets/support"    # or: password_cmd = "pass show sip/support"

[[agent]]
name = "customer"
username = "1002"
domain = "pbx.example.com"
password = "other-secret"
```

At startup the config is validated (names, required fields, passwords
resolved) — a broken file fails fast before any MCP traffic. Nothing else
runs at startup: agents are spawned lazily by the first tool call that uses
them, and a crashed worker is respawned the same way.

See [Configuration](configuration.md) for all fields.

## Trim the tool surface (optional)

Every tool definition costs context in the LLM client — and some setups want
a locked-down surface. Both flags take **group names**
(`discovery`, `call-control`, `audio`, `headers`, `events`, `streams`,
`recording`, `lifecycle`) or individual tool names:

```sh
ringo-mcp --disable call-control            # observe only, no dialing
ringo-mcp --enabled-tools discovery,events  # allowlist: just these
ringo-mcp --disable headers --disable add_header   # groups and tools mix
```

`--enabled-tools` is an allowlist (everything else is disabled);
`--disable` subtracts on top of it. Unknown names fail at startup with the
valid list.

### Restrict what may be dialed

The agent shouldn't be able to dial freely — expensive destinations, premium
numbers. Two regex flags cover every `dial`/`transfer` target, globally for
all agents (repeatable flags; each regex is matched against the dialed number
and the full resolved URI):

```sh
ringo-mcp --dial-deny '^00' --dial-deny '^0900' --dial-allow '^\d{2,5}$'
```

`--dial-deny` always wins over `--dial-allow`; an empty allow list means
unrestricted. A denied call is a tool error naming the matched rule, so the
LLM can adjust. Invalid regexes fail at startup.

### Other flags

The live-audio bridge bind host is also a flag: `--bridge-host <IP>`
(default `127.0.0.1`, loopback only; the port is always ephemeral).

## Wire it into an MCP client

ringo-mcp speaks MCP over stdio. Point your client at the binary:

**Claude Code** (`claude mcp add`):

```sh
claude mcp add ringo -- ringo-mcp
```

**Claude Desktop / Cursor / generic** (`mcpServers` in the client's config):

```json
{
  "mcpServers": {
    "ringo": {
      "command": "ringo-mcp",
      "args": ["--config", "/absolute/path/to/config.toml"]
    }
  }
}
```

**Pi** (via the [pi-mcp-adapter] extension) — global, in
`~/.pi/agent/mcp.json`:

```json
{
  "mcpServers": {
    "ringo": {
      "command": "ringo-mcp",
      "args": ["--config", "/absolute/path/to/config.toml"]
    }
  }
}
```

Agents are only started when a tool actually uses them, so an idle MCP client
holds no SIP registrations.

[pi-mcp-adapter]: https://github.com/badlogic/pi-mono/

## Your first call

Ask your agent to make a call, and it will chain the tools like this:

| Tool | Result |
| ---- | ------ |
| `list_agents` | who is configured (`running: false` = not started yet) |
| `dial` | places the call from an agent (spawns it if needed) |
| `wait_event` | blocks until `call_established` (or `call_closed` with a reason) |
| `send_dtmf` / `play` / `transfer` / `hold` … | act on the call |
| `hangup` | end it |

The equivalent of a hello-world, by hand:

```jsonc
// dial
{ "agent": "support", "target": "0800123456" }

// wait for the outcome (blocking, up to 120s)
{ "agent": "support", "timeout_ms": 30000 }
// → { "event": "call_established", "call_id": "…" }

// done
{ "agent": "support" }
```

Dial targets accept full URIs (`sip:1002@pbx.example.com`), `user@host`, or a
bare number/extension (resolved to `sip:<target>@<agent's domain>`).

## Diagnostics

The MCP stream stays clean; everything else goes to stderr (most clients show
it as server logs). Workers are silent unless you set `RINGO_AGENT_LOG`
(`-` for stderr, a path for per-agent files) — same for
`RINGO_AGENT_SIPTRACE` — in the server's environment; both are inherited by
the worker processes.

To check a setup without an MCP client at all, drive the server by hand:

```sh
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_agents","arguments":{}}}' \
  | ringo-mcp
```
