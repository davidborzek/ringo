// Webhook-driven API test with the built-in HTTP mock server.
//
// The pattern: a telephony API (Twilio-style) calls our webhook for a call and we
// answer with the actions it should perform. We stand up a mock server, point the
// API at it, place a call, and assert the API hit our webhook as expected. The
// mock server is stopped automatically at the end of the scenario.
//
// Environment:
//   SIP_DOMAIN      the SIP domain the caller registers to
//   A_USER A_PASS   the calling account
//   API_URL         base URL of the API under test (receives the config call)
//   API_NUMBER      the number that routes into the API
//
// Run:
//   SIP_DOMAIN=… A_USER=… A_PASS=… API_URL=… API_NUMBER=… \
//     ringo-flow run crates/ringo-flow/examples/webhook-mock.rhai

let dom = env("SIP_DOMAIN");

// Start the mock on a free port; `hooks.url` is the base URL to hand to the API.
let hooks = mock_server();

// Dynamic responder: the API posts a webhook, we answer with the call actions.
// The closure must be pure (request in, response out) — no agent verbs here.
hooks.on("POST", "/voice", |req| {
    if req.json("event") == "incoming_call" {
        json_response(#{ actions: [
            #{ type: "answer" },
            #{ type: "play", url: "https://example.com/greeting.wav" },
        ] })
    } else {
        json_response(#{ actions: [ #{ type: "hangup" } ] })
    }
});

// Routes can match a regex path and/or any HTTP method:
//   - regex(...) matches per-call status callbacks like /calls/<id>/status
//   - omitting the method (3 args → 2 args) matches any method
hooks.on(regex("/calls/.*/status"), |req| text_response("ok"));

// Tell the API under test where to send its webhooks.
http("PUT", env("API_URL") + "/config?webhook=" + hooks.url + "/voice");

let a = agent("A", #{ username: env("A_USER"), domain: dom, password: env("A_PASS") });
a.register();
await_until(|| assert(a.registered).is_true(), "10s");

// Place the call into the API; it should call our webhook back.
a.dial(env("API_NUMBER"));

// Wait for the webhook via the same await_until used everywhere else.
await_until(|| assert(hooks.request_count("/voice")).equals(1), "10s");

// Inspect the recorded request.
let req = hooks.last_request("/voice");
assert(req.json("event")).equals("incoming_call");
assert(req.header("content-type")).contains("application/json");

a.hangup();
await_until(|| assert(a.state).equals(State::Idle), "10s");
