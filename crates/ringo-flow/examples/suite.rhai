// A scenario suite. `setup()` brings up + registers the agents and returns them
// as the context; each `scenario(…)` runs in isolation (fresh agents); the
// optional `teardown(…)` runs after each. Credentials come from the environment.
//
// Scenarios can carry options: `#{ tags: [...], skip: true|"reason", only: true }`.
//
// Run all:    ringo-flow run crates/ringo-flow/examples/suite.rhai
// Run one:    ringo-flow run crates/ringo-flow/examples/suite.rhai --scenario "rejected call"
// By tag:     ringo-flow run crates/ringo-flow/examples/suite.rhai --tag smoke
// Skip a tag: ringo-flow run crates/ringo-flow/examples/suite.rhai --exclude-tag slow

let DOMAIN = env("SIP_DOMAIN");

setup(|| {
    let caller = agent("Caller", #{ username: env("A_USER"), domain: DOMAIN, password: env("A_PASS") });
    let callee = agent("Callee", #{ username: env("B_USER"), domain: DOMAIN, password: env("B_PASS") });
    caller.register();
    callee.register();
    await_until(|| assert(caller.registered).is_true(), "10s");
    await_until(|| assert(callee.registered).is_true(), "10s");
    #{ caller: caller, callee: callee }   // → ctx for each scenario
});

scenario("answered call", #{ tags: ["smoke"] }, |ctx| {
    ctx.caller.dial(ctx.callee);
    await_until(|| assert(ctx.callee.state).equals(State::Ringing), "15s");
    ctx.callee.accept();
    await_until(|| assert(ctx.caller.state).equals(State::Established));
    ctx.caller.hangup();
    await_until(|| assert(ctx.caller.state).equals(State::Idle), "10s");
});

scenario("rejected call", #{ tags: ["smoke"] }, |ctx| {
    ctx.caller.dial(ctx.callee);
    await_until(|| assert(ctx.callee.state).equals(State::Ringing), "15s");
    ctx.callee.hangup();                  // callee rejects
    await_until(|| assert(ctx.caller.state).equals(State::Idle), "10s");
});

// Statically disabled — reported as skipped, never run (until the bug is fixed).
scenario("flaky under load", #{ tags: ["slow"], skip: "needs investigation" }, |ctx| {
    ctx.caller.dial(ctx.callee);
    await_until(|| assert(ctx.callee.state).equals(State::Ringing), "15s");
});
