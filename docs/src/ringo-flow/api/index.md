# API reference

The complete scenario vocabulary, generated from the engine (so it never drifts from the code) — organized by the thing you're working with:

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

New to it? Start with [Your first scenario](../your-first-scenario.md), then [Writing scenarios](../writing-scenarios.md).

For editors and agents, the whole API is also available as [Rhai type definitions](../ringo-flow.d.rhai) (`.d.rhai`) — point the Rhai language server at it for completion and hover.
