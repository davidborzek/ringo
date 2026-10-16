# Getting started

## Install

The SIP stack (baresip, libre, OpenSSL) is built in — no separate `baresip`
install needed.

**Homebrew (macOS / Linux):**

```sh
brew install davidborzek/tap/ringo-mcp
```

**Arch Linux (AUR)** — prebuilt, via any AUR helper:

```sh
yay -S ringo-mcp-bin   # or: paru -S ringo-mcp-bin
```

**Nix (flake):**

```sh
nix profile install github:davidborzek/ringo#ringo-mcp
```

Enable the binary cache first (`cachix use ringo`) so this is a download
instead of a long build; see [ringo-phone's install notes](../ringo-phone/getting-started.md#install)
for NixOS settings and pinning a released version
(`github:davidborzek/ringo/ringo-mcp-v<version>#ringo-mcp`).

**Pre-built binaries** for Linux and macOS (x86\_64 + arm64) are on the
[releases page](https://github.com/davidborzek/ringo/releases) — download,
extract and put `ringo-mcp` on your `$PATH`.

**With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall)** — the
same prebuilt binary, no compile:

```sh
cargo binstall ringo-mcp
```

**From crates.io** (compiles, needs a C toolchain and CMake):

```sh
cargo install ringo-mcp
```

**From GitHub** (no clone needed):

```sh
cargo install --git https://github.com/davidborzek/ringo ringo-mcp
```

**From a workspace checkout** (no install):

```sh
cargo run -p ringo-mcp
```

The pre-built binaries, `cargo binstall` and `cargo install` link the opus
and spandsp libraries at runtime — install them first (`libopus0
libspandsp2` on Debian/Ubuntu, `opus spandsp` with Homebrew or pacman).
Homebrew, the AUR package and Nix pull them in for you.

> Homebrew 6.0+ requires third-party taps to be trusted before use. If `brew
> install` prompts you to trust the tap, accept it — or trust it up front:
>
> ```sh
> brew tap davidborzek/tap
> brew trust --formula davidborzek/tap/ringo-mcp
> ```

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
(`discovery`, `call-control`, `audio`, `headers`, `events`,
`recording`, `lifecycle`, `trace`) or individual tool names:

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
bare number/extension (resolved to `sip:<target>@<agent's domain>`). Numbers
are cleaned up like in ringo-phone: `tel:`/`callto:` and separators
(`+49 89 123-456` → `+4989123456`) are dropped before the dial policy checks
them.

## Diagnostics

The MCP stream stays clean; everything else goes to stderr (most clients show
it as server logs). Workers are silent unless you set `RINGO_AGENT_LOG`
(`-` for stderr, a path for per-agent files) in the server's environment; it
is inherited by the worker processes.

SIP traces are on by default: the workers write them as text to a private
temp dir (`$XDG_RUNTIME_DIR/ringo-mcp-*`, removed when the server exits), and the
LLM reads them with [`sip_trace`](tools.md#sip_trace). To keep them elsewhere,
set `RINGO_AGENT_SIPTRACE` yourself: a `.pcap` path writes a capture for
sngrep/Wireshark, `-` writes to stderr. `sip_trace` only reads text files, so
it reports the trace as unavailable in those cases.

To check a setup without an MCP client at all, drive the server by hand:

```sh
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_agents","arguments":{}}}' \
  | ringo-mcp
```
