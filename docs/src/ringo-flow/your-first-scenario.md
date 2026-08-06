# Your first scenario

Let's write a complete test: two agents place, answer and tear down a call. We'll
build it line by line — every concept you need for most scenarios is here.

You'll need two SIP accounts.

## The whole script

Save this as `first.js`:

```js
// @ts-check
const domain = env("SIP_DOMAIN");

const a = new Agent("A", { username: env("A_USER"), domain, password: env("A_PASS") });
const b = new Agent("B", { username: env("B_USER"), domain, password: env("B_PASS") });

a.register();
b.register();
await until(() => expect(a.registered).toBeTruthy(), "10s");
await until(() => expect(b.registered).toBeTruthy(), "10s");

a.dial(b);
await until(() => expect(b.state).toBe(State.Ringing), "15s");
b.accept();
await until(() => expect(a.state).toBe(State.Established));

await wait(3); // the call must stay up
a.hangup();
await until(() => expect(a.state).toBe(State.Idle), "10s");
```

Run it:

```sh
SIP_DOMAIN=example.com A_USER=alice A_PASS=… B_USER=bob B_PASS=… \
  ringo-flow run first.js
```

## Line by line

**Credentials from the environment.**
[`env("SIP_DOMAIN")`](js-api/functions/env.md) reads a variable, so no secrets
live in the script. Pass them as shown above, or from an
[`--env-file`](running-in-ci.md).

**Create the agents.** [`new Agent(name, { … })`](js-api/classes/Agent.md)
connects a headless baresip instance and returns a handle you drive with verbs.
`name` is just a label used in the log. See the
[Agent](js-api/classes/Agent.md) reference for every config field — with the
[`.d.ts`](ringo-flow.d.ts) in place your editor completes them and flags a typo
before the script ever runs.

**Register, then wait for it.** SIP is asynchronous:
[`register()`](js-api/classes/Agent.md#register) only *starts* registration.
[`await until(() => <assertion>, "10s")`](js-api/functions/until.md) re-runs the
assertion until it holds or the timeout elapses — never `sleep` and hope.
[`expect(a.registered)`](js-api/functions/expect.md) reads the agent's state;
[`.toBeTruthy()`](js-api/interfaces/Assertion.md#tobetruthy) checks it.

**Place the call.** [`a.dial(b)`](js-api/classes/Agent.md#dial) calls B at its
address (you can also dial a number or SIP URI as a string). We then wait until B
is *ringing* — [`b.state`](js-api/classes/Agent.md#state) is one of
`State.Idle` / `State.Ringing` / `State.Established`.

**Answer and connect.** [`b.accept()`](js-api/classes/Agent.md#accept) answers;
both sides become `Established`. `until` without a timeout uses the default
(overridable with [`defaultTimeout(...)`](js-api/functions/defaultTimeout.md)).

**Hold, then hang up.** [`await wait(3)`](js-api/functions/wait.md) holds for
three seconds — and *fails* if an established call drops in that window, so it
doubles as a stability check. [`a.hangup()`](js-api/classes/Agent.md#hangup) ends
the call; we confirm both return to `Idle`.

**What to `await`.** Only the blocking verbs are Promises: `until`, `wait`,
`http`, `verifyAudio` and `verifyAudioConnection`. Everything instant (`dial`,
`accept`, `register`, `hangup`, `dtmf`, …) is a plain synchronous call. A
scenario file runs as an ES module, so top-level `await` — as above — is fine.

## What failure looks like

Assertions report `expect … — actual …`, and the exit code is non-zero if any
assertion fails — so this runs cleanly in CI. Add `-v` to see every assertion, or
`--log` (SIP signaling to stderr, or `--log <file>`) when something's off.

## Next

- [Writing scenarios](writing-scenarios.md) — group several tests into a suite and
  select/tag/skip them.
- [Audio testing](audio.md) — assert what the other side actually hears.
- [HTTP & webhooks](http-and-webhooks.md) — drive and mock a backend API.
- The [JS API reference](js-api/index.md) — every class, verb and matcher.
