// @ts-check
// Three-party blind transfer (SIP REFER).
//
// Caller calls Callee; Callee answers, then transfers the call to Target. baresip
// has the Caller place the new call automatically; once Target answers, Caller and
// Target are talking and Callee has dropped out.
//
// This is a real SIP REFER (`transfer`), not a PBX feature code dialled over DTMF.
// For the attended variant — talk to Target first, then connect the two — use
// `attendedTransfer(target)` followed by `completeTransfer()`.
//
// Environment:
//   SIP_DOMAIN      domain all three accounts register to
//   A_USER A_PASS   Caller (places the call)
//   B_USER B_PASS   Callee (answers, then transfers)
//   C_USER C_PASS   Target (receives the transfer)
//
// Run:
//   SIP_DOMAIN=example.com A_USER=… A_PASS=… B_USER=… B_PASS=… C_USER=… C_PASS=… \
//     ringo-flow run crates/ringo-flow/examples/three-party-transfer.js

defaultTimeout("15s");

/** @type {Agent} */ let caller;
/** @type {Agent} */ let callee;
/** @type {Agent} */ let target;

setup(() => {
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
  target = new Agent("Target", {
    username: env("C_USER"),
    domain,
    password: env("C_PASS"),
  });
});

teardown(() => caller.hangup());

scenario("blind transfer connects caller and target", async () => {
  for (const a of [caller, callee, target]) a.register();
  await Promise.all([
    until(() => expect(caller.registered).toBeTruthy(), "10s"),
    until(() => expect(callee.registered).toBeTruthy(), "10s"),
    until(() => expect(target.registered).toBeTruthy(), "10s"),
  ]);

  // 1) Caller calls Callee; Callee answers → Caller <-> Callee established.
  caller.dial(callee);
  await until(() => expect(callee.state).toBe(State.Ringing));
  callee.accept();
  await Promise.all([
    until(() => expect(caller.state).toBe(State.Established)),
    until(() => expect(callee.state).toBe(State.Established)),
  ]);

  // 2) Callee refers the call to Target; baresip dials Target from the Caller.
  callee.transfer(target);

  // 3) Target rings — the transferred leg arrives from the Caller, not the Callee.
  await until(() => expect(target.state).toBe(State.Ringing));
  log("Target's incoming caller: " + target.peer?.number);
  target.accept();

  // 4) Caller and Target are connected; Callee is out of the call.
  await Promise.all([
    until(() => expect(caller.state).toBe(State.Established)),
    until(() => expect(target.state).toBe(State.Established)),
    until(() => expect(callee.state).toBe(State.Idle), "10s"),
  ]);

  // The transferred call carries real two-way audio, not just signalling.
  await verifyAudioConnection(caller, target);

  await wait(3); // the Caller <-> Target call must stay up
  caller.hangup();
  await Promise.all([
    until(() => expect(caller.state).toBe(State.Idle), "10s"),
    until(() => expect(target.state).toBe(State.Idle), "10s"),
  ]);
});
