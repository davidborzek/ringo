<h1 class="lockup">
  <img class="lockup-mark" src="../logo.svg" alt="" />
  <span class="lockup-text"><span class="lockup-name">ringo</span><span class="lockup-sub">flow</span></span>
</h1>

**ringo-flow** is a declarative telephony scenario test runner for
[baresip](https://github.com/baresip/baresip). You write a scenario as a small
JavaScript or TypeScript file — bring up SIP agents, place and answer calls,
assert on call state, DTMF, audio and HTTP — and run it headlessly, e.g. in CI.

```js
// @ts-check
const domain = env("SIP_DOMAIN");

const a = new Agent("A", { username: env("A_USER"), domain, password: env("A_PASS") });
const b = new Agent("B", { username: env("B_USER"), domain, password: env("B_PASS") });

a.register();
b.register();
await until(() => expect(b.registered).toBeTruthy(), "10s");

a.dial(b);
await until(() => expect(b.state).toBe(State.Ringing), "15s");
b.accept();
await until(() => expect(a.state).toBe(State.Established));
a.hangup();
```

## Highlights

- **Headless** — virtual audio, no devices needed; runs on a build server.
- **Typed, in your editor** — the generated [`ringo-flow.d.ts`](ringo-flow.d.ts)
  types the whole DSL, so agent config keys, matchers and argument types are
  checked as you type. Author in plain `.js` with `// @ts-check`, or in real
  TypeScript.
- **Suites** — `setup` / `scenario` / `teardown`, each scenario isolated with
  fresh agents. Parametrise with `scenario.each`, select with `--scenario`, tag
  with `--tag` / `--exclude-tag`, disable with `skip`, focus with `only`.
- **Audio** — send tones / files and assert what the other side receives
  (Goertzel tone detection).
- **HTTP** — call backend APIs mid-scenario, and stand up a built-in mock server
  to test webhook-driven call control.

> Scenarios used to be written in [Rhai](https://rhai.rs). That frontend still
> runs `.rhai` files but is **deprecated and will be removed** — see
> [Rhai frontend](rhai.md) for the reasons and a migration table.

## Next steps

- [Getting started](getting-started.md) — install and run.
- [Your first scenario](your-first-scenario.md) — a guided, line-by-line walkthrough.
- [Writing scenarios](writing-scenarios.md) — suites, selection, and the patterns.
- [Audio testing](audio.md) and [HTTP & webhooks](http-and-webhooks.md) — the
  feature guides.
- The **JS API reference** (in the sidebar) — every class, verb and matcher,
  generated from the type definitions.

The Rust library API is on [docs.rs](https://docs.rs/ringo-flow).
