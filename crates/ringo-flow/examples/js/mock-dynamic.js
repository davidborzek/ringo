// @ts-check

// Dynamic mock responder: the response is computed per request from the request
// itself. The closure runs on the scenario thread (pumped from until), so it
// can close over scenario state — here a simple hit counter.
scenario("dynamic mock responder", async () => {
  const mock = mockServer({ port: 18099 });

  let hits = 0;
  mock.respond("GET", "/greet", (req) => {
    hits++;
    return {
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ hello: req.query.name || "world", hit: hits }),
    };
  });

  log("mock up at " + mock.url + " — waiting for a request to /greet");

  // An external caller (the SUT) hits the mock; the responder fires from this poll.
  await until(() => expect(mock.requestCount("/greet")).toBeGreaterThan(0), "15s");

  const last = mock.lastRequest("/greet");
  expect(last?.path).toBe("/greet");
  expect(hits).toBeGreaterThan(0);
  log("served " + hits + " dynamic request(s)");
});
