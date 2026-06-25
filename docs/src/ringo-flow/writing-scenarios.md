# Writing scenarios

A scenario is a Rhai script. The top level can be the whole test, or you can
register several named scenarios as a **suite**.

## Agents and call control

[`agent(name, #{ … })`](api/agents.md#agent) connects a headless baresip instance
and returns a handle you drive with verbs — [`register`](api/agents.md#register),
[`dial`](api/agents.md#dial), [`accept`](api/agents.md#accept),
[`hangup`](api/agents.md#hangup), `hold`, `dtmf`, `transfer`, … See
[Agents](api/agents.md) for the full set, the config options and the readable
state ([`registered`](api/agents.md#registered), [`state`](api/agents.md#state), …).

## `await_until`

SIP is asynchronous, so assertions are polled:
[`await_until`](api/flow-and-timing.md#await_until) re-runs an
[`assert(...)`](api/assertions-and-matchers.md#assert) until it holds or a timeout
elapses. Use it instead of sleeping.

```rust
a.dial(b);
await_until(|| assert(b.state).equals(State::Ringing), "15s");
```

The matchers — [`equals`](api/assertions-and-matchers.md#equals),
[`is_true`](api/assertions-and-matchers.md#is_true),
[`contains`](api/assertions-and-matchers.md#contains), … — are all on the assertion
handle.

## Suites: `setup` / `scenario` / `teardown`

[`setup()`](api/scenario-structure.md#setup) runs before each scenario and returns
the context passed to it; each
[`scenario(name, body)`](api/scenario-structure.md#scenario) runs in isolation with
fresh agents; [`teardown()`](api/scenario-structure.md#teardown) runs after each
(even on failure).

```rust
setup(|| {
    let caller = agent("Caller", #{
        username: env("A_USER"),
        domain: env("SIP_DOMAIN"),
        password: env("A_PASS"),
    });
    caller.register();
    await_until(|| assert(caller.registered).is_true(), "10s");
    #{ caller: caller }
});

scenario("answered call", #{ tags: ["smoke"] }, |ctx| {
    ctx.caller.dial("+49301234567");
    await_until(|| assert(ctx.caller.state).equals(State::Established), "15s");
});
```

## Selecting, tagging and skipping

The [`scenario(name, #{ … }, body)`](api/scenario-structure.md#scenario) options
control which scenarios run:

- **Tags** — `#{ tags: ["smoke"] }`, then `--tag smoke` / `--exclude-tag slow`.
- **Skip** — `#{ skip: true | "reason" }` disables a scenario statically; or call
  [`skip("reason")`](api/scenario-structure.md#skip) at runtime (e.g. env-gated).
- **Focus** — `#{ only: true }` runs only the focused scenario(s), run-wide.

Skipped scenarios are reported but don't fail the run.

## More

- [Assertions and matchers](api/assertions-and-matchers.md) — the full matcher set.
- [Audio testing](audio.md) — send tones/files and assert what's received.
- [HTTP & webhooks](http-and-webhooks.md) — call and mock a backend API.
