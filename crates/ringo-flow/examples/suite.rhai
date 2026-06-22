// A scenario suite. `setup()` brings up + registers the agents and returns them
// as the context; each `scenario(…)` runs in isolation (fresh agents); the
// optional `teardown(…)` runs after each. Credentials come from the environment.
//
// Run all:   ringo-flow run crates/ringo-flow/examples/suite.rhai
// Run one:   ringo-flow run crates/ringo-flow/examples/suite.rhai --scenario "rejected call"

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

scenario("answered call", |ctx| {
    ctx.caller.dial(ctx.callee);
    await_until(|| assert(ctx.callee.state).equals(State::Ringing), "15s");
    ctx.callee.accept();
    await_until(|| assert(ctx.caller.state).equals(State::Established));
    ctx.caller.hangup();
    await_until(|| assert(ctx.caller.state).equals(State::Idle), "10s");
});

scenario("rejected call", |ctx| {
    ctx.caller.dial(ctx.callee);
    await_until(|| assert(ctx.callee.state).equals(State::Ringing), "15s");
    ctx.callee.hangup();                  // callee rejects
    await_until(|| assert(ctx.caller.state).equals(State::Idle), "10s");
});
