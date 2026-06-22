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
verify audio — and **asserts** the outcome. Assertions are event-driven: they
wait for the expected state instead of sleeping, and the run exits non-zero on
the first failure. No sound hardware needed; it's built on the shared
[`ringo-core`](../ringo-core) engine.

## Use cases

- **Regression-test a PBX / SIP setup** — registration, call routing, rejection,
  hold, blind & attended transfer, conferences.
- **CI for telephony backends** — fully headless (virtual audio), so it runs on a
  build server with no devices.
- **End-to-end feature checks** — DTMF/IVR navigation, MWI, custom SIP headers,
  and real two-way **audio** (tone detection over the media path).
- **Cross-check a backend API** — make HTTP calls mid-scenario and assert the
  system recorded the call (e.g. a correlation id carried on an inbound INVITE).

## How it works

```rhai
let a = agent("A", #{ username: env("A_USER"), domain: env("SIP_DOMAIN"), password: env("A_PASS") });
let b = agent("B", #{ username: env("B_USER"), domain: env("SIP_DOMAIN"), password: env("B_PASS") });

a.register();
await_until(|| assert(a.registered).is_true());

a.dial(b);
await_until(|| assert(b.state).equals(State::Ringing));
b.accept();
await_until(|| assert(a.state).equals(State::Established));
```

- **Agents** are SIP endpoints you create and drive with verbs (`register`,
  `dial`, `accept`, `hangup`, `hold`, `transfer`, `dtmf`, `send_audio`, …).
- **`assert(x).<matcher>(…)`** is a fluent, auto-labeled check; wrap it in
  **`await_until(|| …)`** to wait for async state to settle (default timeout,
  overridable per call).
- A file with `scenario("name", |ctx| { … })` calls becomes a **suite** — each
  scenario runs in isolation with `setup`/`teardown`; otherwise the whole script
  is a single scenario.
- Credentials come from the **environment** (`env(...)`, `--env-file`, a per-file
  `<scenario>.env`) so secrets stay out of scripts.

It's a normal Rhai script, so variables, `if`/`for` and `fn` definitions all work.
The full verb / getter / matcher list — with signatures — is in the generated
[**scenario API reference**](docs/scenario-api.md).

## Getting started

Requires [baresip](https://github.com/baresip/baresip) >= 3.14 in `$PATH`.

```sh
cargo install --git https://github.com/davidborzek/ringo ringo-flow
# …or from a workspace checkout: cargo run -p ringo-flow -- run scenario.rhai

SIP_DOMAIN=example.com A_USER=alice A_PASS=… B_USER=bob B_PASS=… \
  ringo-flow run scenario.rhai
```

```sh
ringo-flow run scenario.rhai     # one file
ringo-flow run scenarios/        # a directory (all *.rhai, recursively)
ringo-flow check scenario.rhai   # syntax-check only (no baresip)
```

Useful flags: `--scenario <pattern>` (run a subset by name; `re:` for regex),
`--env-file FILE`, `--logs` (print SIP signaling), `--save-audio`, `--json`
(NDJSON for CI), `-q`/`-v`, `--no-color`, `--insecure-http` (skip TLS verification
for `http(...)`). The exit code is non-zero if any
scenario fails. See `ringo-flow run --help` for the full list.

## Docker

A small image (~36 MB) with baresip compiled in — nothing to install, ideal for
CI. The release workflow builds and pushes it to GHCR on each `ringo-flow-v*`
tag, so just pull and run a scenario directory (mounted read-only):

```sh
docker run --rm --network host \
  -e SIP_DOMAIN=example.com -e A_USER=alice -e A_PASS=… -e B_USER=bob -e B_PASS=… \
  -v "$PWD/scenarios:/scn:ro" \
  ghcr.io/davidborzek/ringo-flow:latest run /scn --scenario "answered call"
```

Tags: `:latest` (the newest release) and `:<version>` to pin a specific one
(e.g. `:0.10.0`).

- **`--network host`** is the simplest way to get working SIP/RTP and to reach
  internal services — the container shares the host's network and DNS. On the
  default bridge network, SIP media and split-horizon/VPN DNS often don't work.
- **Credentials:** pass them as `-e VAR=…` for `env(...)`, or mount a dotenv file
  and add `--env-file /scn/dev.env`.
- **Recordings:** `--save-audio` writes to the working dir (`/work`); mount a
  writable volume there to keep them.

**TLS to a private/corporate CA.** If `http(...)` targets a service whose cert is
signed by an internal CA, give the container that trust store — reqwest uses
rustls + rustls-native-certs, which honors `SSL_CERT_FILE`:

```sh
docker run --rm --network host \
  -v /etc/ssl/certs/ca-certificates.crt:/ca.pem:ro -e SSL_CERT_FILE=/ca.pem \
  -v "$PWD/scenarios:/scn:ro" ghcr.io/davidborzek/ringo-flow:latest run /scn
```

Without it, such requests fail with *unable to get local issuer certificate*. As a
last resort, `--insecure-http` (or `RINGO_FLOW_INSECURE_HTTP=1`) skips certificate
verification entirely — only for throwaway dev testing.

To build the image yourself (for development):
`docker build -f crates/ringo-flow/Dockerfile -t ringo-flow .`

## Examples

[`examples/`](examples/) has runnable, commented scenarios:

- [`two-party.rhai`](examples/two-party.rhai) — two agents place, answer and tear
  down a call.
- [`suite.rhai`](examples/suite.rhai) — a suite with `setup`/`teardown` and an
  answered- vs rejected-call scenario.
- [`three-party-transfer.rhai`](examples/three-party-transfer.rhai) — three
  agents and a blind **SIP REFER**: Callee transfers the Caller to a Target, who
  ends up connected while the Callee drops out.

## API reference & editor support

The API is generated from the engine, so it never drifts from the code. The
canonical reference is [**docs/scenario-api.md**](docs/scenario-api.md):

```sh
ringo-flow docs docs/scenario-api.md                # Markdown reference (default)
ringo-flow docs ringo-flow-api.html --format html   # self-contained HTML page
ringo-flow definitions ringo-flow.d.rhai            # Rhai definition file (LSP)
```

Point the [Rhai language server](https://github.com/rhaiscript/lsp) at the
`.d.rhai` for completion, signatures and hover docs in your editor.

## Notes

- **`wait(n)` is a hold, not a sleep** — it fails if a call that's established at
  the start drops during it.
- **DTMF:** for reliable headless DTMF set `dtmf_mode: "info"` on the agent. (RTP
  telephone-event needs a clocked TX, which idles once a headless agent's audio
  goes silent, so only the first digit reaches the wire.)
- **Audio** is verified headless via baresip's `aubridge` + Goertzel tone
  detection; for a conference give each party a distinct tone and check each one
  hears the others.

## Security

Scenario files are **trusted code**, not sandboxed input: a scenario can make
arbitrary HTTP requests (`http(...)`) and read local files (`file(...)`,
`load_env(...)`). Only run scenarios you wrote or reviewed — and in CI, where the
runner has network reach and real credentials, keep scenario sources and env
files under the same review controls as the rest of your code.
