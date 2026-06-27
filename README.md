# ringo

> A terminal SIP softphone and telephony test runner built on [baresip](https://github.com/baresip/baresip).

[![CI](https://github.com/davidborzek/ringo/actions/workflows/ci.yml/badge.svg)](https://github.com/davidborzek/ringo/actions/workflows/ci.yml)
[![ringo-phone on crates.io](https://img.shields.io/crates/v/ringo-phone?label=ringo-phone)](https://crates.io/crates/ringo-phone)
[![ringo-flow on crates.io](https://img.shields.io/crates/v/ringo-flow?label=ringo-flow)](https://crates.io/crates/ringo-flow)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

**ringo** is a cargo workspace of telephony tools built on baresip — a SIP
softphone you live in, and a scenario runner that drives baresip headless for
automated call testing, both sharing one engine.

📖 **Documentation: https://davidborzek.github.io/ringo/**

## Tools

| Crate | What it is | Docs |
|-------|------------|------|
| [`ringo-phone`](crates/ringo-phone) | The **`ringo` softphone** — a ratatui TUI managing multiple SIP profiles, with calls, contacts, transfers, remote control and headless automation | [Guide](https://davidborzek.github.io/ringo/ringo-phone/introduction.html) |
| [`ringo-flow`](crates/ringo-flow) | A **telephony scenario test runner** — bring up SIP agents from a [Rhai](https://rhai.rs) script, drive them, and assert call behaviour (incl. audio) | [Guide](https://davidborzek.github.io/ringo/ringo-flow/introduction.html) · [API](https://davidborzek.github.io/ringo/ringo-flow/api/scenario-structure.html) |
| [`ringo-core`](crates/ringo-core) | The **shared engine** — an FFI backend statically linking baresip/libre, the call-event model (internal, no stable API) | — |

## Requirements

- **Rust 1.85+** to build
- A **C toolchain + CMake** to build the vendored [baresip](https://github.com/baresip/baresip)/libre/OpenSSL, which are **statically linked** — so neither the softphone nor the test runner needs a separate `baresip` install, at build or run time

## Quick start

Install the softphone and open the profile picker:

```sh
cargo install ringo-phone   # installs the `ringo` binary
ringo                       # Ctrl+N to create your first profile
```

See the [documentation](https://davidborzek.github.io/ringo/) for install
options, configuration, remote control and writing scenario tests with
[ringo-flow](https://davidborzek.github.io/ringo/ringo-flow/introduction.html).

## Development

The repo is a cargo workspace; build and test all crates together:

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
```

Contributions are welcome. Please open an issue before submitting large changes
so we can discuss the approach first.

## License

MIT — see [LICENSE](LICENSE).
