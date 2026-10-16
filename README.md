<h1 align="center">
  <img src="docs/src/logo.svg" alt="" width="72"><br>
  ringo
</h1>

<p align="center">A terminal SIP softphone, a telephony test runner and an MCP server for LLM-driven calls, built on <a href="https://github.com/baresip/baresip">baresip</a>.</p>

<p align="center">
  <a href="https://github.com/davidborzek/ringo/actions/workflows/ci.yml"><img src="https://github.com/davidborzek/ringo/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/ringo-phone"><img src="https://img.shields.io/crates/v/ringo-phone?label=ringo-phone" alt="ringo-phone on crates.io"></a>
  <a href="https://crates.io/crates/ringo-flow"><img src="https://img.shields.io/crates/v/ringo-flow?label=ringo-flow" alt="ringo-flow on crates.io"></a>
  <a href="https://crates.io/crates/ringo-mcp"><img src="https://img.shields.io/crates/v/ringo-mcp?label=ringo-mcp" alt="ringo-mcp on crates.io"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.85%2B-orange.svg" alt="Rust 1.85+"></a>
</p>

**ringo** is a cargo workspace of SIP telephony tools sharing one engine: a
softphone you live in, a scenario runner that drives SIP agents headless for
automated call testing, and an MCP server that hands those agents to an LLM.

📖 **Documentation: https://davidborzek.github.io/ringo/**

![ringo-phone: alice picks her profile, calls Bob, then Charlie while Bob is on hold, switches back and hangs up](docs/src/assets/ringo-phone.gif)

## Tools

| Crate | What it is | Docs |
|-------|------------|------|
| [`ringo-phone`](crates/ringo-phone) | The **`ringo` softphone** — a ratatui TUI managing multiple SIP profiles, with calls, contacts, transfers, remote control and headless automation | [Guide](https://davidborzek.github.io/ringo/ringo-phone/introduction.html) |
| [`ringo-flow`](crates/ringo-flow) | A **telephony scenario test runner** — bring up SIP agents from a JavaScript/TypeScript scenario, drive them, and assert call behaviour (incl. audio) | [Guide](https://davidborzek.github.io/ringo/ringo-flow/introduction.html) · [API](https://davidborzek.github.io/ringo/ringo-flow/js-api/) |
| [`ringo-mcp`](crates/ringo-mcp) | An **MCP server** — exposes SIP agents as tools to LLM clients (dial, DTMF, transfer, INVITE headers, audio playback, recording) | [Guide](https://davidborzek.github.io/ringo/ringo-mcp/introduction.html) |

## Internal crates

Support crates with no stable public API — pin an exact version if you depend on them directly.

| Crate | What it is |
|-------|------------|
| [`ringo-agent`](crates/ringo-agent) | The **per-agent process backend** — runs a SIP user agent as its own process over a framed stdio protocol (live audio in/out); backs `ringo-flow` and `ringo-mcp` |
| [`ringo-core`](crates/ringo-core) | The **shared engine** — an FFI backend statically linking baresip/libre, plus the call-event model |
| [`ringo-profile`](crates/ringo-profile) | The **profile file format** and custom-header templates — shared by `ringo-phone` and `ringo-mcp` |

## Install

The SIP stack (baresip, libre, OpenSSL) is built in — no separate `baresip`
install needed.

| | ringo-phone | ringo-flow | ringo-mcp |
|---|---|---|---|
| Homebrew | `brew install davidborzek/tap/ringo` | `brew install davidborzek/tap/ringo-flow` | `brew install davidborzek/tap/ringo-mcp` |
| AUR | `ringo-phone-bin` | `ringo-flow-bin` | `ringo-mcp-bin` |
| Nix | `nix profile install github:davidborzek/ringo#ringo-phone` | `…#ringo-flow` | `…#ringo-mcp` |
| cargo | `cargo binstall ringo-phone` | `cargo binstall ringo-flow` | `cargo binstall ringo-mcp` |
| Docker | — | `ghcr.io/davidborzek/ringo-flow` | — |

Pre-built binaries for Linux and macOS (x86\_64 + arm64) are on the
[releases page](https://github.com/davidborzek/ringo/releases). The
getting-started pages
([phone](https://davidborzek.github.io/ringo/ringo-phone/getting-started.html),
[flow](https://davidborzek.github.io/ringo/ringo-flow/getting-started.html),
[mcp](https://davidborzek.github.io/ringo/ringo-mcp/getting-started.html)) have
the details: the Nix binary cache, runtime libraries for the plain binaries, and
`cargo install` from source.

Then run `ringo` and press <kbd>Ctrl</kbd>+<kbd>N</kbd> to create your first
profile.

## Development

Building from source needs **Rust 1.85+** and a **C toolchain + CMake** for the
vendored baresip/libre/OpenSSL, which are statically linked. The repo is a cargo
workspace; build and test all crates together:

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
```

See [BUILD.md](BUILD.md) for the full build setup — git submodules, system
dependencies, and the vendored/static linking of libre, libbaresip and OpenSSL.

Contributions are welcome. Please open an issue before submitting large changes
so we can discuss the approach first.

## License

MIT — see [LICENSE](LICENSE).
