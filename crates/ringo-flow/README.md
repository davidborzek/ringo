# ringo-flow

[![crates.io](https://img.shields.io/crates/v/ringo-flow)](https://crates.io/crates/ringo-flow)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> Declarative telephony tests for [baresip](https://github.com/baresip/baresip):
> script SIP call scenarios and assert what happens.

> [!WARNING]
> **The scenario API is not stable yet.** ringo-flow shares the workspace version
> and is still pre-1.0 (`0.x`); verbs, getters, output and behaviour may change in
> **breaking** ways between releases. Pin an exact version if you depend on it.

ringo-flow runs automated **call tests**. A scenario is a [Rhai](https://rhai.rs)
script that brings up one or more SIP **agents** (each a headless baresip
instance), drives them — register, dial, accept, transfer, send DTMF, play and
verify audio, call webhooks — and **asserts** the outcome. Assertions are
event-driven: they wait for the expected state instead of sleeping, and the run
exits non-zero on the first failure. No sound hardware needed; it's built on the
shared [`ringo-core`](../ringo-core) engine.

📖 **Full documentation: https://davidborzek.github.io/ringo/ringo-flow/introduction.html**
— a guide (your first scenario, writing scenarios, audio testing, HTTP &
webhooks, running in CI) and the generated
[**scenario API reference**](https://davidborzek.github.io/ringo/ringo-flow/api/scenario-structure.html).

## Requirements

**Rust 1.85+** and a **C toolchain + CMake** to build the vendored
baresip/libre/OpenSSL, which are **statically linked** — so no separate `baresip`
install is needed, at build or run time. For CI there's also a small self-contained
Docker image (`ghcr.io/davidborzek/ringo-flow`) — see
[Running in CI](https://davidborzek.github.io/ringo/ringo-flow/running-in-ci.html).

## Install

```sh
brew install davidborzek/tap/ringo-flow   # Homebrew (macOS/Linux)
cargo install --git https://github.com/davidborzek/ringo ringo-flow
```

## Getting started

```rhai
// scenario.rhai
let a = agent("A", #{ username: env("A_USER"), domain: env("SIP_DOMAIN"), password: env("A_PASS") });
let b = agent("B", #{ username: env("B_USER"), domain: env("SIP_DOMAIN"), password: env("B_PASS") });

a.register();
await_until(|| assert(a.registered).is_true());

a.dial(b);
await_until(|| assert(b.state).equals(State::Ringing));
b.accept();
await_until(|| assert(a.state).equals(State::Established));
```

```sh
SIP_DOMAIN=example.com A_USER=alice A_PASS=… B_USER=bob B_PASS=… \
  ringo-flow run scenario.rhai

ringo-flow run scenarios/        # a directory (all *.rhai, recursively)
ringo-flow check scenario.rhai   # syntax-check only (no SIP traffic)
```

The [**Your first scenario**](https://davidborzek.github.io/ringo/ringo-flow/your-first-scenario.html)
walkthrough explains this line by line. See the guide for tags & filtering,
audio verification, the HTTP mock server, Docker/CI and the full API.
Runnable examples live in [`examples/`](https://github.com/davidborzek/ringo/tree/main/crates/ringo-flow/examples).

### Editor support

The API is generated from the engine, so it never drifts from the code. Emit a
Rhai definition file and point the [Rhai language server](https://github.com/rhaiscript/lsp)
at it for completion, signatures and hover docs:

```sh
ringo-flow definitions ringo-flow.d.rhai
```

## Security

Scenario files are **trusted code**, not sandboxed input: a scenario can make
arbitrary HTTP requests (`http(...)`) and read local files (`file(...)`,
`load_env(...)`). Only run scenarios you wrote or reviewed — and in CI, where the
runner has network reach and real credentials, keep scenario sources and env
files under the same review controls as the rest of your code.

## License

MIT — see [LICENSE](../../LICENSE).
