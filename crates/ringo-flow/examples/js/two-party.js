// @ts-check
// A real two-party scenario showcase (compiles + wires through to baresip; the
// dial/accept legs need live SIP creds via env to actually pass). Run with:
//   ringo-flow run crates/ringo-flow/examples/js/two-party.js
//
// Fixtures are plain closure-scoped variables: declared once up top, assigned in
// `setup()` (which runs fresh before each scenario), and read by every scenario /
// teardown. The `@type` annotations give every body full type-checking + completion
// with no per-scenario typing — the idiomatic JS test-fixture pattern.

/** @type {Agent} */ let caller;
/** @type {Agent} */ let callee;

setup(() => {
  // Connect two real baresip agents; `agent(...)` returns the live handle.
  caller = agent("caller", {
    username: env("A_USER"),
    domain: env("SIP_DOMAIN"),
    password: env("A_PASS"),
  });
  callee = agent("callee", {
    username: env("B_USER"),
    domain: env("SIP_DOMAIN"),
    password: env("B_PASS"),
  });
});

teardown(() => caller.hangup());

scenario("answered call", async () => {
  // Instant verbs (dial/accept/register/hangup) stay synchronous; only the
  // blocking waiters (until/verifyAudio) are Promises you `await`.
  caller.register();
  await until(() => expect(caller.registered).toBeTruthy(), "10s");

  caller.dial(callee);
  await until(() => expect(callee.state).toBe(State.Ringing), "15s");
  callee.accept();
  await until(() => expect(caller.state).toBe(State.Established), "10s");

  expect(caller.quality?.mos).toBeGreaterThanOrEqual(4.0);

  // Both legs send a tone and we verify reception on both ends concurrently.
  caller.sendAudio(tone(440));
  callee.sendAudio(tone(480));
  await Promise.all([callee.verifyAudio(440, "5s"), caller.verifyAudio(480, "5s")]);
});
