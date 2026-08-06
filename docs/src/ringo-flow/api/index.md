# API reference (Rhai — deprecated)

> **The Rhai frontend is deprecated and will be removed in a future release.** New scenarios should use the JavaScript/TypeScript frontend — see [Writing scenarios](../writing-scenarios.md) and the [JS API reference](../js-api/index.md). This page documents the Rhai vocabulary for existing scenarios; [Rhai frontend](../rhai.md) explains why and how to migrate.

The complete Rhai scenario vocabulary, generated from the engine (so it never drifts from the code) — organized by the thing you're working with:

- [Scenario structure](scenario-structure.md) — defining and isolating tests: `scenario`, `setup`, `teardown`, `skip`.
- [Flow and timing](flow-and-timing.md) — `await_until`, `wait`, `parallel`, `default_timeout`.
- [Agents](agents.md) — create SIP endpoints and drive calls: register, dial, accept, transfer, DTMF, audio.
  - [Peer](peer.md) — the remote party of the active call.
  - [Call state](call-state.md) — the `State::*` phases for `agent.state`.
  - [AudioSpec](audiospec.md) — audio sources for `send_audio` (`tone`, `file`, `silent`).
- [CallQuality](callquality.md)
- [Assertions and matchers](assertions-and-matchers.md) — the fluent `assert(x).<matcher>(…)`, used inside `await_until`.
- [HTTP](http.md) — `http(…)` requests and the response.
- [HTTP mock server](http-mock-server.md) — `mock_server(…)`, routes and responders for webhook-driven flows.
  - [Mock request](mock-request.md) — the recorded request a responder/assertion sees.
- [Environment](environment.md) — `env`, `load_env` — credentials stay out of scripts.
- [Utilities](utilities.md) — `log`, `uuid`.

For editors and agents, the whole Rhai API is also available as [Rhai type definitions](../ringo-flow.d.rhai) (`.d.rhai`). In practice this is a reference you read, not tooling you get: the only Rhai language server is an unreleased experiment that no editor plugin ships, so there is no type-checking and no inline error reporting — one of the reasons the frontend is being retired in favour of JS/TS.
