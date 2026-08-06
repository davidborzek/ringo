# ringo-flow

[![crates.io](https://img.shields.io/crates/v/ringo-flow)](https://crates.io/crates/ringo-flow)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> Declarative telephony tests for [baresip](https://github.com/baresip/baresip):
> script SIP call scenarios and assert what happens.

> [!WARNING]
> **The scenario API is not stable yet.** ringo-flow shares the workspace version
> and is still pre-1.0 (`0.x`); verbs, getters, output and behaviour may change in
> **breaking** ways between releases. Pin an exact version if you depend on it.

ringo-flow runs automated **call tests**. A scenario is a JavaScript (or
TypeScript) file that brings up one or more SIP **agents** (each a headless
baresip instance), drives them — register, dial, accept, transfer, send DTMF, play and
verify audio, call webhooks — and **asserts** the outcome. Assertions are
event-driven: they wait for the expected state instead of sleeping, and the run
exits non-zero on the first failure. No sound hardware needed; it's built on the
shared [`ringo-core`](../ringo-core) engine.

📖 **Full documentation: https://davidborzek.github.io/ringo/ringo-flow/introduction.html**
— a guide (your first scenario, writing scenarios, audio testing, HTTP &
webhooks, running in CI) and the generated
[**scenario API reference**](https://davidborzek.github.io/ringo/ringo-flow/js-api/).

## Requirements

**Rust 1.85+** and a **C toolchain + CMake** to build the vendored
baresip/libre/OpenSSL, which are **statically linked** — so no separate `baresip`
install is needed, at build or run time. For CI there's also a small self-contained
Docker image (`ghcr.io/davidborzek/ringo-flow`) — see
[Running in CI](https://davidborzek.github.io/ringo/ringo-flow/running-in-ci.html).

## Install

```sh
brew install davidborzek/tap/ringo-flow   # Homebrew (macOS/Linux)
yay -S ringo-flow-bin                      # Arch (AUR, prebuilt)
cargo install --git https://github.com/davidborzek/ringo ringo-flow
```

## Getting started

```js
// scenario.js
// @ts-check
const a = new Agent("A", { username: env("A_USER"), domain: env("SIP_DOMAIN"), password: env("A_PASS") });
const b = new Agent("B", { username: env("B_USER"), domain: env("SIP_DOMAIN"), password: env("B_PASS") });

a.register();
await until(() => expect(a.registered).toBeTruthy());

a.dial(b);
await until(() => expect(b.state).toBe(State.Ringing));
b.accept();
await until(() => expect(a.state).toBe(State.Established));
```

```sh
SIP_DOMAIN=example.com A_USER=alice A_PASS=… B_USER=bob B_PASS=… \
  ringo-flow run scenario.js

ringo-flow run scenarios/      # a directory (all *.js, recursively)
ringo-flow check scenario.js   # syntax-check only (no SIP traffic)
```

The [**Your first scenario**](https://davidborzek.github.io/ringo/ringo-flow/your-first-scenario.html)
walkthrough explains this line by line. See the guide for tags & filtering,
audio verification, the HTTP mock server, Docker/CI and the full API.
Runnable examples live in [`examples/`](https://github.com/davidborzek/ringo/tree/main/crates/ringo-flow/examples).

### Editor support

The API is generated from the engine, so it never drifts from the code. Emit the
TypeScript definitions next to your scenarios and any editor with TypeScript
support type-checks the whole DSL — config keys, matchers, argument types — as you
type:

```sh
ringo-flow definitions --lang js ringo-flow.d.ts
```

Start a scenario with `// @ts-check` to get the same errors from `tsc --noEmit`,
or author in real TypeScript and transpile — see
[Writing scenarios](https://davidborzek.github.io/ringo/ringo-flow/writing-scenarios.html).

### Rhai (deprecated)

Scenarios used to be written in [Rhai](https://rhai.rs). `.rhai` files still run,
but the frontend is **deprecated and will be removed** — no usable IDE tooling
(the Rhai LSP is an unreleased experiment, last touched in 2022), missing language
features and an unfamiliar syntax. See
[Rhai frontend](https://davidborzek.github.io/ringo/ringo-flow/rhai.html) for the
reasons and a migration table.

## Security

Scenario files are **trusted code**, not sandboxed input: a scenario can make
arbitrary HTTP requests (`http(...)`) and read local files (`file(...)`,
`loadEnv(...)`). Only run scenarios you wrote or reviewed — and in CI, where the
runner has network reach and real credentials, keep scenario sources and env
files under the same review controls as the rest of your code.

## License

MIT — see [LICENSE](../../LICENSE).
