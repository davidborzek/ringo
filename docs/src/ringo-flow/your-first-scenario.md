# Your first scenario

Let's write a complete test: two agents place, answer and tear down a call. We'll
build it line by line — every concept you need for most scenarios is here.

You'll need two SIP accounts.

## The whole script

Save this as `first.rhai`:

```rust
let dom = env("SIP_DOMAIN");

let a = agent("A", #{
    username: env("A_USER"),
    domain: dom,
    password: env("A_PASS"),
});
let b = agent("B", #{
    username: env("B_USER"),
    domain: dom,
    password: env("B_PASS"),
});

a.register();
b.register();
await_until(|| assert(a.registered).is_true(), "10s");
await_until(|| assert(b.registered).is_true(), "10s");

a.dial(b);
await_until(|| assert(b.state).equals(State::Ringing), "15s");
b.accept();
await_until(|| assert(a.state).equals(State::Established));

wait(3); // the call must stay up
a.hangup();
await_until(|| assert(a.state).equals(State::Idle), "10s");
```

Run it:

```sh
SIP_DOMAIN=example.com A_USER=alice A_PASS=… B_USER=bob B_PASS=… \
  ringo-flow run first.rhai
```

## Line by line

**Credentials from the environment.** [`env("SIP_DOMAIN")`](api/environment.md#env)
reads a variable, so no secrets live in the script. Pass them as shown above, or
from an [`--env-file`](running-in-ci.md).

**Create the agents.** [`agent(name, #{ … })`](api/agents.md#agent) connects a
headless baresip instance and returns a handle you drive with verbs. `name` is just a label
used in the log. See the [Agents](api/agents.md) reference for every config field.

**Register, then wait for it.** SIP is asynchronous: [`register()`](api/agents.md#register)
only *starts* registration. [`await_until(|| <assertion>, "10s")`](api/flow-and-timing.md#await_until)
re-runs the assertion until it holds or the timeout elapses — never `sleep` and
hope. [`assert(a.registered)`](api/assertions-and-matchers.md#assert) reads the
agent's state; [`.is_true()`](api/assertions-and-matchers.md#is_true) checks it.

**Place the call.** [`a.dial(b)`](api/agents.md#dial) calls B at its address (you
can also dial a number or SIP URI as a string). We then wait until B is *ringing*
— [`b.state`](api/agents.md#state) is one of `State::Idle` / `State::Ringing` /
`State::Established`.

**Answer and connect.** [`b.accept()`](api/agents.md#accept) answers; both sides
become `Established`. `await_until` without a timeout uses the default (overridable
with [`default_timeout(...)`](api/flow-and-timing.md#default_timeout)).

**Hold, then hang up.** [`wait(3)`](api/flow-and-timing.md#wait) holds for three
seconds — and *fails* if an established call drops in that window, so it doubles as
a stability check. [`a.hangup()`](api/agents.md#hangup) ends the call; we confirm
both return to `Idle`.

## What failure looks like

Assertions report `expect … — actual …`, and the exit code is non-zero if any
assertion fails — so this runs cleanly in CI. Add `-v` to see every assertion, or
`--log` (SIP signaling to stderr, or `--log <file>`) when something's off.

## Next

- [Writing scenarios](writing-scenarios.md) — group several tests into a suite and
  select/tag/skip them.
- [Audio testing](audio.md) — assert what the other side actually hears.
- [HTTP & webhooks](http-and-webhooks.md) — drive and mock a backend API.
- The [API reference](api/agents.md) — every verb, getter and matcher.
