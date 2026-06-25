# HTTP & webhooks

Telephony rarely lives alone — there's usually a backend that records calls or
drives them. ringo-flow can both **call** an HTTP API mid-scenario and **mock**
one your system under test calls back.

## Call an API

[`http(method, url)`](api/http.md#http) makes a request and returns a response you
can assert on:

```rust
let res = http("GET", env("API_URL") + "/calls/last");
res.expect_status(200);
assert(res.json("from")).equals("+49301234567");
```

[`res.json("a.b.0.c")`](api/http.md#json) walks a dotted JSON path;
[`res.status`](api/http.md#status) / [`res.body`](api/http.md#body) /
[`res.header(name)`](api/http.md#header) are there too. For requests with headers
or a body, pass an options map — see [HTTP](api/http.md):

```rust
http("POST", env("API_URL") + "/calls", #{
    headers: #{ "Content-Type": "application/json" },
    body: #{ to: "+49301234567" },
});
```

## Mock a webhook (webhook-driven call control)

Some telephony APIs call *your* webhook for a call and expect you to answer with
the actions to perform. Stand up a built-in mock server, point the API at it, and
assert on what it received.

[`mock_server()`](api/http-mock-server.md#mock_server) starts the server;
[`on(...)`](api/http-mock-server.md#on) answers a route dynamically,
[`json_response`](api/http-mock-server.md#json_response) builds the body, and
[`last_request`](api/http-mock-server.md#last_request) /
[`request_count`](api/http-mock-server.md#request_count) inspect what arrived:

```rust
let hooks = mock_server();

// Answer the webhook with the call actions to perform.
hooks.on("POST", "/voice", |req| {
    if req.json("event") == "incoming_call" {
        json_response(#{ actions: [ #{ type: "answer" } ] })
    } else {
        json_response(#{ actions: [ #{ type: "hangup" } ] })
    }
});

// Tell the system under test where to send its webhooks.
http("PUT", env("API_URL") + "/config?webhook=" + hooks.url + "/voice");

a.dial(env("API_NUMBER"));

// Wait for the webhook the same way you wait for anything else.
await_until(|| assert(hooks.request_count("/voice")).equals(1), "10s");

let req = hooks.last_request("/voice");
assert(req.json("event")).equals("incoming_call");
```

Notes:

- The `on(...)` responder runs on a worker thread, so keep it pure (request →
  response): no agent verbs inside it.
- Routes match by exact path or [`regex("/calls/.*")`](api/http-mock-server.md#regex),
  and by a method or any (`"*"` / omit the method). Re-register a route with
  [`respond(...)`](api/http-mock-server.md#respond) to stage the next answer between
  webhooks.
- The server is stopped automatically at the end of the scenario.

See the [HTTP mock server](api/http-mock-server.md) and
[Mock request](api/mock-request.md) reference for everything.
