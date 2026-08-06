# HTTP & webhooks

Telephony rarely lives alone — there's usually a backend that records calls or
drives them. ringo-flow can both **call** an HTTP API mid-scenario and **mock**
one your system under test calls back.

## Call an API

[`http(method, url)`](js-api/functions/http.md) performs the request off-thread
and resolves with a response you can assert on:

```js
const res = await http("GET", env("API_URL") + "/calls/last");
res.expectStatus(200);
expect(res.json("from")).toBe("+49301234567");
```

[`res.json("a.b.0.c")`](js-api/interfaces/HttpResponse.md#json) walks a dotted
JSON path and returns a native JS value;
[`res.status`](js-api/interfaces/HttpResponse.md#status) /
[`res.body`](js-api/interfaces/HttpResponse.md#body) /
[`res.header(name)`](js-api/interfaces/HttpResponse.md#header) are there too. For
requests with headers or a body, pass an options object — an object `body` is
JSON-encoded for you:

```js
await http("POST", env("API_URL") + "/calls", {
  headers: { "Content-Type": "application/json" },
  body: { to: "+49301234567" },
});
```

Since each request is a Promise, `Promise.all` fires several at once:

```js
const [a, b] = await Promise.all([
  http("GET", env("API_URL") + "/calls/1"),
  http("GET", env("API_URL") + "/calls/2"),
]);
```

## Mock a webhook (webhook-driven call control)

Some telephony APIs call *your* webhook for a call and expect you to answer with
the actions to perform. Stand up a built-in mock server, point the API at it, and
assert on what it received.

[`new MockServer()`](js-api/classes/MockServer.md) starts the server;
[`respond(...)`](js-api/classes/MockServer.md#respond) answers a route (statically
or from a per-request closure),
[`jsonResponse`](js-api/functions/jsonResponse.md) builds the body, and
[`lastRequest`](js-api/classes/MockServer.md#lastrequest) /
[`requestCount`](js-api/classes/MockServer.md#requestcount) inspect what arrived:

```js
const hooks = new MockServer();

// Answer the webhook with the call actions to perform.
hooks.respond("POST", "/voice", (req) => {
  const event = JSON.parse(req.body).event;
  return event === "incoming_call"
    ? jsonResponse({ actions: [{ type: "answer" }] })
    : jsonResponse({ actions: [{ type: "hangup" }] });
});

// Tell the system under test where to send its webhooks.
await http("PUT", env("API_URL") + "/config?webhook=" + hooks.url + "/voice");

a.dial(env("API_NUMBER"));

// Wait for the webhook the same way you wait for anything else.
await until(() => expect(hooks.requestCount("/voice")).toBe(1), "10s");

const req = hooks.lastRequest("/voice");
expect(JSON.parse(req?.body ?? "{}").event).toBe("incoming_call");
```

Notes:

- The responder closure runs on the **scenario thread**, pumped from `until`, so
  it may close over scenario state (a counter, a flag) — but it must stay pure
  request → response: no agent verbs and no `await` inside it. A scenario that
  never reaches an `until` also never serves a request.
- The request it receives is `{ method, path, query, headers, body }` — `body` is
  the raw string, so parse it with `JSON.parse` (unlike the HTTP *response*,
  which has a `json(path)` helper).
- Routes match by exact path or
  [`regex("/calls/.*")`](js-api/functions/regex.md), and by a method or any
  (`"*"` / omit the method). Re-register a route with `respond(...)` to stage the
  next answer between webhooks.
- A static object works where you don't need the request:
  `hooks.respond("/health", { status: 204 })`, or
  `hooks.respond("/config", jsonResponse({ ok: true }))`.
- The server is stopped automatically at the end of the scenario;
  [`stop()`](js-api/classes/MockServer.md#stop) ends it early.

See the [MockServer](js-api/classes/MockServer.md) and
[MockRequestInfo](js-api/interfaces/MockRequestInfo.md) reference for everything.
