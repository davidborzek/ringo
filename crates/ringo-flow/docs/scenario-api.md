# ringo-flow scenario API

Functions, getters and types available in a `.rhai` scenario. **Generated** from the engine with `ringo-flow docs` — do not edit by hand; see the [README](../README.md) for concepts and usage.

## Top-level

### `await_until(body: Fn) -> ?`

Re-run the expression until its assertion holds or the default timeout
elapses: `await_until(|| assert(a.registered).is_true())`. Returns the
body's value, so `.value()` can bind a verified value.

### `await_until(body: Fn, within: string) -> ?`

Like `await_until(body)` but with an explicit timeout, e.g. `"15s"`.

### `default_timeout(duration: string)`

Set the default `await_until` timeout for the rest of the script (e.g. `"10s"`).

### `env(name: string) -> string`

Read a variable: first from `--env-file`/`<scenario>.env`/`load_env`, then
the process environment. Errors if unset. Use it for per-env credentials.

### `load_env(path: string)`

Load a dotenv file (`KEY=VALUE` lines) into `env(...)` for this scenario,
resolved relative to the scenario file. Later loads override earlier keys.

### `log(message: string)`

Print a timestamped note to the scenario log (and the `--json` stream),
unlike `print` which writes a bare line.

### `parallel(tasks: array) -> array`

Run the given zero-arg closures concurrently and wait for all; returns
their results as an array, and fails if any task fails. Use it for
independent blocking work, e.g. `verify_audio` on several agents at once.
Tasks may share captured variables (each gets an independent snapshot,
so they can't race). Don't overlap `await_until` across tasks; its
silencing is global.

### `regex(pattern: string) -> PathPattern`

A regex path matcher for `respond`/`on`/`request_count`/… anchored to the
whole path: `regex("/calls/.*")` matches `/calls/123`. Errors on a bad
pattern.

### `scenario(name: string, body: Fn)`

Register a named scenario, run in isolation (fresh agents, torn down
after). The body may take the `setup()` context: `|ctx| { … }`.

### `setup(body: Fn)`

Run before each scenario; its return value is passed to the scenario
(and teardown) as `ctx`. Typically creates and registers the agents.

### `teardown(body: Fn)`

Run after each scenario (even on failure); receives the `setup` context.

### `to_string(state: CallState) -> string`

The call state as a string.

### `uuid() -> string`

A fresh random UUID string.

### `wait(seconds: int)`

Hold for N seconds; FAILS if a call that is established at the start drops.

## Agents

### `abort_transfer(agent: Agent)`

Abort the pending attended transfer.

### `accept(agent: Agent)`

Answer the agent's incoming call.

### `agent(name: string, config: map) -> Agent`

Connect a headless baresip agent and return a handle.
`config` is a map: `username`/`domain` (required), `password`, `display_name`,
`transport`, `auth_user`, `outbound`, `stun_server`, `media_enc`, `regint`,
`mwi`, `dtmf_mode` (`"info"` for reliable headless DTMF), `headers`.

### `attended_transfer(agent: Agent, target: Agent)`

Start an attended transfer: place a consultation call to another agent.
Complete it with `complete_transfer()` once that call is established.

### `attended_transfer(agent: Agent, target: string)`

Start an attended transfer to a literal URI or bare number.

### `complete_transfer(agent: Agent)`

Complete the pending attended transfer (REFER with Replaces).

### `dial(agent: Agent, target: Agent)`

Dial another agent at its AOR.

### `dial(agent: Agent, target: string)`

Dial a literal SIP URI, or a bare number/extension in the agent's own domain.

### `dtmf(agent: Agent, digits: string)`

Send DTMF tones (characters `0-9`, `*`, `#`, `A-D`) back-to-back.

### `dtmf(agent: Agent, digits: string, gap: string)`

Send DTMF tones with a pause between digits, e.g. `dtmf("123#", "200ms")`.

### `get name(peer: Peer) -> ?`

The remote party's display name, or `()` if absent.

### `get number(peer: Peer) -> ?`

The remote party's number (user-part of the URI), or `()`.

### `get peer(agent: Agent) -> Peer`

The current call's remote party (the caller for an incoming call); read
`peer.uri` / `peer.number` / `peer.name` (each `()` if there's no call).

### `get reason(agent: Agent) -> ?`

The last closed call's reason (string), or `()` if none yet.

### `get registered(agent: Agent) -> bool`

Whether the agent's account is currently registered.

### `get state(agent: Agent) -> CallState`

The agent's current call phase: `Idle`, `Ringing` or `Established`.

### `get status_code(agent: Agent) -> ?`

SIP status code from the last closed call's reason (int, e.g. `603`),
or `()` if the reason isn't a SIP response (local hangup, reset, …).

### `get uri(peer: Peer) -> ?`

The remote party's full URI (e.g. `sip:bob@example.com`), or `()`.

### `hangup(agent: Agent)`

Hang up the agent's active call.

### `header(agent: Agent, name: string) -> ?`

Value of a header on a received INVITE (string), or `()` if absent.

### `headers(agent: Agent) -> map`

All received INVITE headers as a map (name → value); duplicates collapse,
use `header(name)` for a specific one.

### `hold(agent: Agent)`

Put the active call on hold.

### `info(agent: Agent) -> map`

A map of the agent's current state: name, aor, registered, state,
reason, status_code, calls. Handy to `print(...)` or assert on.

### `mute(agent: Agent)`

Toggle mute on the active call.

### `register(agent: Agent)`

(Re-)register the agent's account.

### `resume(agent: Agent)`

Resume a held call.

### `to_json(agent: Agent) -> string`

The agent's current state as a JSON string (for `log(...)`/debugging).

### `transfer(agent: Agent, target: Agent)`

Blind-transfer (REFER) the active call to another agent's AOR.

### `transfer(agent: Agent, target: string)`

Blind-transfer (REFER) the active call to a literal URI or bare number.

## Assertions & matchers

### `assert(actual) -> Assertion`

Begin a fluent assertion on a value: `assert(x).equals(y)`, `.is_true()`,
`.greater_than(n)`, etc. Matchers chain (`.at_least(200).at_most(299)`)
and error (with a value-based message) on a mismatch. Asserting on a
getter auto-labels the log line (`assert(caller.state)` → `Caller state`,
`assert(res.status)` → `HTTP status`); `.describe(…)` overrides.

### `at_least(a: Assertion, n: int) -> Assertion`

Assert the (numeric) value is >= `n`.

### `at_most(a: Assertion, n: int) -> Assertion`

Assert the (numeric) value is <= `n`.

### `contains(a: Assertion, needle: string) -> Assertion`

Assert the (string) value contains `needle`.

### `describe(a: Assertion, label: string) -> Assertion`

Label this assertion so the log line names it: `assert(caller.registered)
.describe("caller registered").is_true()` → `caller registered: ✓ expect …`.

### `equals(a: Assertion, expected) -> Assertion`

Assert the value equals `expected` (`is` is a reserved word in Rhai).

### `greater_than(a: Assertion, n: int) -> Assertion`

Assert the (numeric) value is > `n`.

### `is_absent(a: Assertion) -> Assertion`

Assert the value is absent (`()`).

### `is_empty(a: Assertion) -> Assertion`

Assert the string/array/map value is empty.

### `is_false(a: Assertion) -> Assertion`

Assert the value is `false`.

### `is_not_empty(a: Assertion) -> Assertion`

Assert the string/array/map value is not empty.

### `is_present(a: Assertion) -> Assertion`

Assert the value is present (not `()`), e.g. a received header.

### `is_true(a: Assertion) -> Assertion`

Assert the value is `true`.

### `less_than(a: Assertion, n: int) -> Assertion`

Assert the (numeric) value is < `n`.

### `matches(a: Assertion, pattern: string) -> Assertion`

Assert the (string) value matches the regex `pattern`.

### `not_equals(a: Assertion, expected) -> Assertion`

Assert the value does not equal `expected`.

### `value(a: Assertion) -> ?`

The value under assertion, so a verified value can be bound:
`let id = await_until(|| assert(callee.header("X-Id")).is_present().value());`.

## HTTP

### `expect_status(response: HttpResponse, code: int)`

Assert and report the status; errors on mismatch.

### `get body(response: HttpResponse) -> string`

The HTTP response body as a string.

### `get status(response: HttpResponse) -> int`

The HTTP response status code.

### `header(response: HttpResponse, name: string) -> ?`

A response header value (string), or `()` if absent.

### `http(method: string, url: string) -> HttpResponse`

Make an HTTP request and return the response.

### `http(method: string, url: string, options: map) -> HttpResponse`

Make an HTTP request with options `#{ headers: #{…}, body: … }`.
`body` may be a string or a map (encoded to JSON).

### `json(response: HttpResponse) -> ?`

The whole JSON body as a native value (object→map, array, …).

### `json(response: HttpResponse, path: string) -> ?`

The value at a dotted JSON path (e.g. `"data.id"`), typed: object→map,
array, number, bool, `null`→`()`. Errors if the path is missing.

## HTTP mock server

### `get body(request: MockRequest) -> string`

The raw request body.

### `get method(request: MockRequest) -> string`

The request method (upper-case).

### `get path(request: MockRequest) -> string`

The request path.

### `get port(mock: HttpMock) -> int`

The port the server is listening on.

### `get url(mock: HttpMock) -> string`

The server's base URL, e.g. `http://127.0.0.1:8080`.

### `header(request: MockRequest, name: string) -> ?`

A request header value (case-insensitive), or `()` if absent.

### `json(request: MockRequest, path: string) -> ?`

The value at a dotted JSON path in the body (object→map, array, number,
bool, `null`→`()`). Errors if the path is missing.

### `json_response(body) -> map`

Build a `200 application/json` response map from `body` (JSON-encoded),
for `respond`/`on`. `body` may be a map or an array, e.g.
`json_response(#{ actions: [ … ] })` or `json_response([ … ])`.

### `last_request(mock: HttpMock, path: PathPattern) -> MockRequest`

The most recent request on a `regex(...)` path (errors if none yet).

### `last_request(mock: HttpMock, path: string) -> MockRequest`

The most recent request on `path` (errors if none yet). Read it after
`await_until` confirms the webhook arrived.

### `mock_server() -> HttpMock`

Start a mock HTTP server on a free port and return a handle. Stopped
automatically at the end of the scenario. Use `url` to point the system
under test at it, `respond`/`on` to define routes.

### `mock_server(config: map) -> HttpMock`

Start a mock HTTP server with config `#{ port: 8080 }` (omit `port` for a
free one). Returns a handle; stopped automatically at scenario end.

### `on(mock: HttpMock, method: string, path: PathPattern, responder: Fn)`

Dynamic responder for `method` and a `regex(...)` path.

### `on(mock: HttpMock, method: string, path: string, responder: Fn)`

Answer `method path` dynamically: the `|req|` closure receives the
`MockRequest` and returns a response map (e.g. `json_response(#{…})`).
`method` may be `"*"` for any method. The closure runs on a runtime
worker, so keep it pure (request → response): no agent verbs, no `wait`
— those block a worker thread.

### `on(mock: HttpMock, path: PathPattern, responder: Fn)`

Dynamic responder for a `regex(...)` path on any HTTP method.

### `on(mock: HttpMock, path: string, responder: Fn)`

Dynamic responder for `path` on any HTTP method.

### `query(request: MockRequest, name: string) -> ?`

A query-string parameter value, or `()` if absent.

### `request_count(mock: HttpMock, path: PathPattern) -> int`

How many requests arrived on a `regex(...)` path (any method).

### `request_count(mock: HttpMock, path: string) -> int`

How many requests arrived on `path` (any method). Poll it with
`await_until`, e.g.
`await_until(|| assert(hooks.request_count("/voice")).equals(1))`.

### `requests(mock: HttpMock, path: PathPattern) -> array`

All requests on a `regex(...)` path, in arrival order, as `MockRequest`s.

### `requests(mock: HttpMock, path: string) -> array`

All requests received on `path`, in arrival order, as `MockRequest`s.

### `respond(mock: HttpMock, method: string, path: PathPattern, response: map)`

Static response for `method` and a `regex(...)` path.

### `respond(mock: HttpMock, method: string, path: string, response: map)`

Register a static response for `method path`: a map
`#{ status: 200, content_type: "…", headers: #{…}, body: <string|map> }`
(use `json_response`/`text_response` to build it). `method` may be `"*"`
for any method. Re-register to stage the next answer between webhooks.

### `respond(mock: HttpMock, path: PathPattern, response: map)`

Static response for a `regex(...)` path on any HTTP method.

### `respond(mock: HttpMock, path: string, response: map)`

Static response for `path` on any HTTP method.

### `stop(mock: HttpMock)`

Stop the server now (it otherwise stops automatically at scenario end).

### `text_response(body: string) -> map`

Build a `200 text/plain` response map from `body`, for `respond`/`on`.

## Audio

### `file(path: string) -> AudioSpec`

A WAV-file audio source, for `send_audio`.

### `send_audio(agent: Agent, source: AudioSpec)`

Switch the agent's active-call audio source: `tone(Hz)`, `file(path)` or `silent()`.

### `silent() -> AudioSpec`

A silent audio source (stop sending), for `send_audio`.

### `tone(freq: int) -> AudioSpec`

A sine-tone audio source at the given frequency (Hz), for `send_audio`.

### `verify_audio(agent: Agent, freq: int, within: string)`

Assert the agent is receiving a tone at `freq` Hz within the window (Goertzel).

### `verify_audio_connection(a: Agent, b: Agent)`

Assert two-way audio between two agents (a→b then b→a) at 1000 Hz.

