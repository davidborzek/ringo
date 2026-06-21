// Three-party blind transfer (SIP REFER).
//
// Caller calls Callee; Callee answers, then blind-transfers the call to Target
// (a SIP REFER). baresip has the Caller automatically call Target; once Target
// answers, Caller and Target are connected and Callee drops out of the call.
//
// This uses a real SIP REFER (the `transfer` verb) — as opposed to a PBX
// feature-code transfer driven by DTMF.
//
// Credentials come from the environment (nothing sensitive in the file):
//   SIP_DOMAIN              domain all three accounts register to
//   A_USER A_PASS           Caller  (places the call)
//   B_USER B_PASS           Callee  (answers, then transfers)
//   C_USER C_PASS           Target  (receives the transfer)
//
// Run:
//   SIP_DOMAIN=example.com \
//   A_USER=… A_PASS=… B_USER=… B_PASS=… C_USER=… C_PASS=… \
//     ringo-flow run crates/ringo-flow/examples/three-party-transfer.rhai

let dom = env("SIP_DOMAIN");
default_timeout("15s");

let caller = agent("Caller", #{ username: env("A_USER"), domain: dom, password: env("A_PASS") });
let callee = agent("Callee", #{ username: env("B_USER"), domain: dom, password: env("B_PASS") });
let target = agent("Target", #{ username: env("C_USER"), domain: dom, password: env("C_PASS") });

caller.register();
callee.register();
target.register();
await_until(|| assert(caller.registered).is_true(), "10s");
await_until(|| assert(callee.registered).is_true(), "10s");
await_until(|| assert(target.registered).is_true(), "10s");

// 1) Caller calls Callee; Callee answers → Caller <-> Callee established.
caller.dial(callee);
await_until(|| assert(callee.state).equals(State::Ringing));
callee.accept();
await_until(|| assert(caller.state).equals(State::Established));
await_until(|| assert(callee.state).equals(State::Established));

// 2) Callee blind-transfers the call to Target (SIP REFER). baresip then makes
//    the Caller place a new call to Target automatically.
callee.transfer(target);

// 3) Target rings — the transferred leg comes from the Caller.
await_until(|| assert(target.state).equals(State::Ringing));
log("Target's incoming caller: " + target.peer.number);
target.accept();

// 4) Caller and Target are now connected; Callee has dropped out of the call.
await_until(|| assert(caller.state).equals(State::Established));
await_until(|| assert(target.state).equals(State::Established));
await_until(|| assert(callee.state).equals(State::Idle), "10s");

// The transferred call carries real two-way audio.
verify_audio_connection(caller, target);

wait(3); // the Caller <-> Target call must stay up
caller.hangup();
await_until(|| assert(caller.state).equals(State::Idle), "10s");
await_until(|| assert(target.state).equals(State::Idle), "10s");
