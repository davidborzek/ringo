// Two-party call test (Rhai scenario).
//
// Credentials come from the environment so nothing sensitive lives in the file:
//   SIP_DOMAIN      the SIP domain both accounts register to
//   A_USER A_PASS   account A
//   B_USER B_PASS   account B
//
// Run:
//   SIP_DOMAIN=… A_USER=… A_PASS=… B_USER=… B_PASS=… \
//     ringo-flow run crates/ringo-flow/examples/two-party.rhai

let dom = env("SIP_DOMAIN");

let a = agent("A", #{ username: env("A_USER"), domain: dom, password: env("A_PASS") });
let b = agent("B", #{ username: env("B_USER"), domain: dom, password: env("B_PASS") });

a.register();
await_until(|| assert(a.registered).is_true(), "10s");
b.register();
await_until(|| assert(b.registered).is_true(), "10s");

a.dial(b); // dial B at its AOR
await_until(|| assert(b.state).equals(State::Ringing), "15s");
b.accept();
await_until(|| assert(a.state).equals(State::Established));
await_until(|| assert(b.state).equals(State::Established));

wait(3); // the call must stay up (fails if it drops)
a.hangup();
await_until(|| assert(a.state).equals(State::Idle), "10s");
await_until(|| assert(b.state).equals(State::Idle), "10s");
