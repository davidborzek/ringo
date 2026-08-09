// @ts-check
// Webhook-driven API test with the built-in HTTP mock server.
//
// The pattern: a telephony API calls our webhook when a call arrives and we answer
// with the actions it should perform. We stand up a mock, point the API at it,
// place a call, and assert the API hit the webhook as expected. The mock stops
// automatically when the scenario ends.
//
// Environment:
//   SIP_DOMAIN      the SIP domain the caller registers to
//   A_USER A_PASS   the calling account
//   API_URL         base URL of the API under test (receives the config call)
//   API_NUMBER      the number that routes into the API
//
// Run:
//   SIP_DOMAIN=… A_USER=… A_PASS=… API_URL=… API_NUMBER=… \
//     ringo-flow run crates/ringo-flow/examples/webhook-mock.js

scenario("the API calls our webhook and we drive the call", async () => {
  // Port 0 (the default) picks a free one; `mock.url` is the base URL to hand out.
  const mock = new MockServer();

  // Dynamic responder: the reply is computed from the request. It runs on the
  // scenario thread (pumped by `until`), so it may close over scenario state —
  // but keep it request-in/response-out, no agent verbs.
  mock.respond("POST", "/voice", (req) => {
    const event = JSON.parse(req.body || "{}").event;
    return event === "incoming_call"
      ? jsonResponse({
          actions: [
            { type: "answer" },
            { type: "play", url: "https://example.com/greeting.wav" },
          ],
        })
      : jsonResponse({ actions: [{ type: "hangup" }] });
  });

  // Routes can match a regex path, and omitting the method matches any of them —
  // handy for per-call status callbacks like /calls/<id>/status.
  mock.respond(regex("/calls/.*/status"), () => textResponse("ok"));

  // Tell the API under test where to send its webhooks. `http` is async.
  await http("PUT", `${env("API_URL")}/config?webhook=${mock.url}/voice`);

  const caller = new Agent("A", {
    username: env("A_USER"),
    domain: env("SIP_DOMAIN"),
    password: env("A_PASS"),
  });
  caller.register();
  await until(() => expect(caller.registered).toBeTruthy(), "10s");

  // Place the call into the API; it should call our webhook back.
  caller.dial(env("API_NUMBER"));
  await until(() => expect(mock.requestCount("/voice")).toBe(1), "10s");

  // Inspect what arrived. Unlike the agent/HTTP-response helpers, a recorded
  // request is plain data: `body` is the raw string, `headers` a lookup map.
  const last = mock.lastRequest("/voice");
  expect(last).toBeDefined();
  expect(JSON.parse(last?.body || "{}").event).toBe("incoming_call");
  expect(last?.headers["content-type"]).toContain("application/json");

  caller.hangup();
  await until(() => expect(caller.state).toBe(State.Idle), "10s");
});
