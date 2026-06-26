# HTTP mock server

## Constructor

<a id="mock_server"></a>

### mock_server()

**Returns** [`HttpMock`](http-mock-server.md)

Start a mock HTTP server on a free port and return a handle. Stopped
automatically at the end of the scenario. Use `url` to point the system
under test at it, `respond`/`on` to define routes.

**Example**

```rust
let hooks = mock_server();
hooks.on("POST", "/voice", |req| json_response(#{ actions: [ #{ type: "answer" } ] }));
http("PUT", env("API_URL") + "/config?webhook=" + hooks.url + "/voice");
```

<a id="mock_server"></a>

### mock_server(config: map)

**Returns** [`HttpMock`](http-mock-server.md)

Start a mock HTTP server with config; stopped automatically at scenario
end. Omit `port` (or use `mock_server()`) for a free one.

**Config** — `mock_server(#{ … })`:

| Field | Type | Description |
| --- | --- | --- |
| `port` | int | port to bind (omit for a free one) |

**Example**

```rust
let hooks = mock_server(#{ port: 8080 });
```

## Methods

<a id="last_request"></a>

### mock.last_request(path: PathPattern)

**Receiver** [`HttpMock`](http-mock-server.md) · **Takes** [`PathPattern`](http-mock-server.md#regex) · **Returns** [`MockRequest`](mock-request.md)

The most recent request on a `regex(...)` path (errors if none yet).

<a id="last_request"></a>

### mock.last_request(path: string)

**Receiver** [`HttpMock`](http-mock-server.md) · **Returns** [`MockRequest`](mock-request.md)

The most recent request on `path` (errors if none yet). Read it after
`await_until` confirms the webhook arrived.

**Example**

```rust
let req = hooks.last_request("/voice");
assert(req.json("event")).equals("incoming_call");
```

<a id="on"></a>

### mock.on(method: string, path: PathPattern, responder: Fn)

**Receiver** [`HttpMock`](http-mock-server.md) · **Takes** [`PathPattern`](http-mock-server.md#regex)

Dynamic responder for `method` and a `regex(...)` path.

<a id="on"></a>

### mock.on(method: string, path: string, responder: Fn)

**Receiver** [`HttpMock`](http-mock-server.md)

Answer `method path` dynamically: the `|req|` closure receives the
`MockRequest` and returns a response map (e.g. `json_response(#{…})`).
`method` may be `"*"` for any method. The closure runs on a runtime
worker, so keep it pure (request → response): no agent verbs, no `wait`
— those block a worker thread.

**Example**

```rust
hooks.on("POST", "/voice", |req| {
    if req.json("event") == "incoming_call" {
        json_response(#{ actions: [ #{ type: "answer" } ] })
    } else {
        json_response(#{ actions: [ #{ type: "hangup" } ] })
    }
});
```

<a id="on"></a>

### mock.on(path: PathPattern, responder: Fn)

**Receiver** [`HttpMock`](http-mock-server.md) · **Takes** [`PathPattern`](http-mock-server.md#regex)

Dynamic responder for a `regex(...)` path on any HTTP method.

<a id="on"></a>

### mock.on(path: string, responder: Fn)

**Receiver** [`HttpMock`](http-mock-server.md)

Dynamic responder for `path` on any HTTP method.

<a id="request_count"></a>

### mock.request_count(path: PathPattern)

**Receiver** [`HttpMock`](http-mock-server.md) · **Takes** [`PathPattern`](http-mock-server.md#regex) · **Returns** `int`

How many requests arrived on a `regex(...)` path (any method).

<a id="request_count"></a>

### mock.request_count(path: string)

**Receiver** [`HttpMock`](http-mock-server.md) · **Returns** `int`

How many requests arrived on `path` (any method). Poll it with
`await_until` to wait for a webhook.

**Example**

```rust
await_until(|| assert(hooks.request_count("/voice")).equals(1), "10s");
```

<a id="requests"></a>

### mock.requests(path: PathPattern)

**Receiver** [`HttpMock`](http-mock-server.md) · **Takes** [`PathPattern`](http-mock-server.md#regex) · **Returns** `array`

All requests on a `regex(...)` path, in arrival order, as `MockRequest`s.

<a id="requests"></a>

### mock.requests(path: string)

**Receiver** [`HttpMock`](http-mock-server.md) · **Returns** `array`

All requests received on `path`, in arrival order, as `MockRequest`s.

<a id="respond"></a>

### mock.respond(method: string, path: PathPattern, response: map)

**Receiver** [`HttpMock`](http-mock-server.md) · **Takes** [`PathPattern`](http-mock-server.md#regex)

Static response for `method` and a `regex(...)` path.

<a id="respond"></a>

### mock.respond(method: string, path: string, response: map)

**Receiver** [`HttpMock`](http-mock-server.md)

Register a static response for `method path`: a map
`#{ status: 200, content_type: "…", headers: #{…}, body: <string|map> }`
(use `json_response`/`text_response` to build it). `method` may be `"*"`
for any method. Re-register to stage the next answer between webhooks.

**Example**

```rust
hooks.respond("POST", "/voice", json_response(#{ actions: [ #{ type: "hangup" } ] }));
```

<a id="respond"></a>

### mock.respond(path: PathPattern, response: map)

**Receiver** [`HttpMock`](http-mock-server.md) · **Takes** [`PathPattern`](http-mock-server.md#regex)

Static response for a `regex(...)` path on any HTTP method.

<a id="respond"></a>

### mock.respond(path: string, response: map)

**Receiver** [`HttpMock`](http-mock-server.md)

Static response for `path` on any HTTP method.

<a id="stop"></a>

### mock.stop()

**Receiver** [`HttpMock`](http-mock-server.md)

Stop the server now (it otherwise stops automatically at scenario end).

## Fields

<a id="port"></a>

### mock.port

**Receiver** [`HttpMock`](http-mock-server.md) · **Returns** `int`

The port the server is listening on.

<a id="url"></a>

### mock.url

**Receiver** [`HttpMock`](http-mock-server.md) · **Returns** `string`

The server's base URL, e.g. `http://127.0.0.1:8080`.

## Helpers

<a id="json_response"></a>

### json_response(body)

**Returns** `map`

Build a `200 application/json` response map from `body` (JSON-encoded),
for `respond`/`on`. `body` may be a map or an array.

**Example**

```rust
hooks.respond("POST", "/voice", json_response(#{ actions: [ #{ type: "answer" } ] }));
```

<a id="regex"></a>

### regex(pattern: string)

**Returns** [`PathPattern`](http-mock-server.md#regex)

A regex path matcher for `respond`/`on`/`request_count`/… anchored to the
whole path (`/calls/.*` matches `/calls/123`). Errors on a bad pattern.

**Example**

```rust
hooks.on(regex("/calls/.*"), |req| text_response("ok"));
```

<a id="text_response"></a>

### text_response(body: string)

**Returns** `map`

Build a `200 text/plain` response map from `body`, for `respond`/`on`.

