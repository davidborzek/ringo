// @ts-check
// A scenario suite: several named tests in one file, each run in isolation with
// fresh agents. `setup()` runs before every scenario, `teardown()` after.
//
// Options select what runs:
//   tags: ["smoke"]            filter with --tag / --exclude-tag
//   skip: "reason"             reported as skipped, never executed
//   only: true                 if any scenario sets it, only those run
//
// Environment:
//   SIP_DOMAIN      the SIP domain both accounts register to
//   A_USER A_PASS   the calling account
//   B_USER B_PASS   the answering account
//
// Run all:     ringo-flow run crates/ringo-flow/examples/suite.js
// Run one:     ringo-flow run crates/ringo-flow/examples/suite.js --scenario "rejected call"
// By tag:      ringo-flow run crates/ringo-flow/examples/suite.js --tag smoke
// Skip a tag:  ringo-flow run crates/ringo-flow/examples/suite.js --exclude-tag slow

// Fixtures as closure-scoped variables: assigned in `setup`, read by every
// scenario, and fully type-checked. `setup` can instead return an object, which
// arrives as each body's `ctx` argument — but that context is untyped, so the
// variables are usually the better trade.
/** @type {Agent} */ let caller;
/** @type {Agent} */ let callee;

setup(async () => {
  const domain = env("SIP_DOMAIN");
  caller = new Agent("Caller", {
    username: env("A_USER"),
    domain,
    password: env("A_PASS"),
  });
  callee = new Agent("Callee", {
    username: env("B_USER"),
    domain,
    password: env("B_PASS"),
  });
  caller.register();
  callee.register();
  await Promise.all([
    until(() => expect(caller.registered).toBeTruthy(), "10s"),
    until(() => expect(callee.registered).toBeTruthy(), "10s"),
  ]);
});

// Runs after every scenario, including failed ones — leave no call up.
teardown(() => caller.hangup());

scenario("answered call", { tags: ["smoke"] }, async () => {
  caller.dial(callee);
  await until(() => expect(callee.state).toBe(State.Ringing), "15s");
  callee.accept();
  await until(() => expect(caller.state).toBe(State.Established), "10s");
  caller.hangup();
  await until(() => expect(caller.state).toBe(State.Idle), "10s");
});

scenario("rejected call", { tags: ["smoke"] }, async () => {
  caller.dial(callee);
  await until(() => expect(callee.state).toBe(State.Ringing), "15s");
  callee.hangup(); // callee rejects instead of answering
  await until(() => expect(caller.state).toBe(State.Idle), "10s");
});

scenario("busy callee is rejected with 486", { tags: ["smoke"] }, async () => {
  // Answer inbound INVITEs with a status instead of picking up.
  callee.respondIncoming(486, "Busy Here");
  caller.dial(callee);
  await until(() => expect(caller.statusCode).toBe(486), "10s");
  await until(() => expect(caller.state).toBe(State.Idle), "10s");
});

// Statically disabled: reported as skipped, never run.
scenario(
  "flaky under load",
  { tags: ["slow"], skip: "needs investigation" },
  async () => {
    caller.dial(callee);
    await until(() => expect(callee.state).toBe(State.Ringing), "15s");
  },
);

// Skipping can also be decided at run time — useful when a scenario needs an
// optional account or a feature the environment may not have.
scenario("voicemail deposit", { tags: ["slow"] }, async () => {
  if (!env("VOICEMAIL_NUMBER")) skip("VOICEMAIL_NUMBER not configured");
  caller.dial(env("VOICEMAIL_NUMBER"));
  await until(() => expect(caller.state).toBe(State.Established), "15s");
  caller.sendAudio(tone(440));
  await wait(2);
  caller.hangup();
});
