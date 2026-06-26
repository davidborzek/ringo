# Scenario structure

<a id="scenario"></a>

## scenario(name: string, body: Fn)

Register a named scenario, run in isolation (fresh agents, torn down
after). The body may take the `setup()` context: `|ctx| { … }`.

**Example**

```rust
scenario("answered call", |ctx| {
    ctx.caller.dial(ctx.callee);
    await_until(|| assert(ctx.callee.state).equals(State::Ringing), "15s");
    ctx.callee.accept();
});
```

<a id="scenario"></a>

## scenario(name: string, options: map, body: Fn)

Register a scenario with options `#{ tags: ["smoke"], skip: true|"reason",
only: true }`. `--tag`/`--exclude-tag` filter by tag; a skipped scenario is
reported but not run; if any scenario sets `only`, only those run.

**Example**

```rust
scenario("smoke: answered", #{ tags: ["smoke"] }, |ctx| {
    ctx.caller.dial(ctx.callee);
    ctx.callee.accept();
});
```

<a id="setup"></a>

## setup(body: Fn)

Run before each scenario; its return value is passed to the scenario
(and teardown) as `ctx`. Typically creates and registers the agents.

**Example**

```rust
setup(|| {
    let caller = agent("Caller", #{ username: env("A_USER"), domain: env("SIP_DOMAIN"), password: env("A_PASS") });
    caller.register();
    #{ caller: caller }
});
```

<a id="skip"></a>

## skip()

Skip the current scenario at runtime (reported, not failed).

<a id="skip"></a>

## skip(reason: string)

Skip the current scenario at runtime with a reason (reported, not failed).

**Example**

```rust
if env("STAGE") != "prod" { skip("prod only") }
```

<a id="teardown"></a>

## teardown(body: Fn)

Run after each scenario (even on failure); receives the `setup` context.

**Example**

```rust
teardown(|ctx| { ctx.caller.hangup(); });
```

