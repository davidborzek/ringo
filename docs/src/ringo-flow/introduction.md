<h1 class="lockup">
  <img class="lockup-mark" src="../logo.svg" alt="" />
  <span class="lockup-text"><span class="lockup-name">ringo</span><span class="lockup-sub">flow</span></span>
</h1>

**ringo-flow** is a declarative telephony scenario test runner for
[baresip](https://github.com/baresip/baresip). You write a scenario as a small
[Rhai](https://rhai.rs) script — bring up SIP agents, place and answer calls,
assert on call state, DTMF, audio and HTTP — and run it headlessly, e.g. in CI.

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
await_until(|| assert(b.registered).is_true(), "10s");

a.dial(b);
await_until(|| assert(b.state).equals(State::Ringing), "15s");
b.accept();
await_until(|| assert(a.state).equals(State::Established));
a.hangup();
```

## Highlights

- **Headless** — virtual audio, no devices needed; runs on a build server.
- **Suites** — `setup` / `scenario` / `teardown`, each scenario isolated with
  fresh agents. Select with `--scenario`, tag with `--tag` / `--exclude-tag`,
  disable with `skip`, focus with `only`.
- **Audio** — send tones / files and assert what the other side receives
  (Goertzel tone detection).
- **HTTP** — call backend APIs mid-scenario, and stand up a built-in mock server
  to test webhook-driven call control.

## Next steps

- [Getting started](getting-started.md) — install and run.
- [Your first scenario](your-first-scenario.md) — a guided, line-by-line walkthrough.
- [Writing scenarios](writing-scenarios.md) — suites, selection, and the patterns.
- [Audio testing](audio.md) and [HTTP & webhooks](http-and-webhooks.md) — the
  feature guides.
- The **API** section (in the sidebar) — every verb, getter and matcher,
  generated from the engine.

The Rust library API is on [docs.rs](https://docs.rs/ringo-flow).
