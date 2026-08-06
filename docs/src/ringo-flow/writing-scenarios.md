# Writing scenarios

A scenario is a JavaScript file (or TypeScript, transpiled — see
[below](#writing-scenarios-in-typescript)). The top level can be the whole test,
or you can register several named scenarios as a **suite**.

Each file runs as an ES module, so you can `import` helper files and use
top-level `await`.

## Agents and call control

[`new Agent(name, { … })`](js-api/classes/Agent.md) connects a headless baresip
instance and returns a handle you drive with verbs —
[`register`](js-api/classes/Agent.md#register),
[`dial`](js-api/classes/Agent.md#dial),
[`accept`](js-api/classes/Agent.md#accept),
[`hangup`](js-api/classes/Agent.md#hangup), `hold`, `dtmf`, `transfer`, … See
[Agent](js-api/classes/Agent.md) for the full set, the config options and the
readable state ([`registered`](js-api/classes/Agent.md#registered),
[`state`](js-api/classes/Agent.md#state), …).

## `until`

SIP is asynchronous, so assertions are polled:
[`until`](js-api/functions/until.md) re-runs an
[`expect(...)`](js-api/functions/expect.md) until it holds or a timeout elapses.
Use it instead of sleeping.

```js
a.dial(b);
await until(() => expect(b.state).toBe(State.Ringing), "15s");
```

The matchers — [`toBe`](js-api/interfaces/Assertion.md#tobe),
[`toBeTruthy`](js-api/interfaces/Assertion.md#tobetruthy),
[`toContain`](js-api/interfaces/Assertion.md#tocontain), … — are all on the
assertion handle, and are the Jest names, so they should already be familiar.

`until` resolves with the value the condition returned, which lets you bind a
verified value in one step:

```js
const traceId = await until(() => expect(b.header("X-Trace-Id")).toBeDefined().value());
```

Because `until` yields the event loop while it polls, independent waiters can run
concurrently:

```js
await Promise.all([callee.verifyAudio(440, "5s"), caller.verifyAudio(480, "5s")]);
```

## Suites: `setup` / `scenario` / `teardown`

[`setup()`](js-api/functions/setup.md) runs before each scenario; each
[`scenario(name, body)`](js-api/functions/scenario.md) runs in isolation with
fresh agents; [`teardown()`](js-api/functions/teardown.md) runs after each (even
on failure).

Keep fixtures in **closure-scoped variables**: declare them once up top, assign
them in `setup()`, and read them everywhere. With a single `@type` annotation per
fixture, every body gets completion and type-checking, with no per-scenario
typing:

```js
// @ts-check
/** @type {Agent} */ let caller;
/** @type {Agent} */ let callee;

setup(() => {
  caller = new Agent("caller", { username: env("A_USER"), domain: env("SIP_DOMAIN"), password: env("A_PASS") });
  callee = new Agent("callee", { username: env("B_USER"), domain: env("SIP_DOMAIN"), password: env("B_PASS") });
});

teardown(() => caller.hangup());

scenario("answered call", { tags: ["smoke"] }, async () => {
  caller.dial(callee);
  await until(() => expect(callee.state).toBe(State.Ringing), "15s");
  callee.accept();
  await until(() => expect(caller.state).toBe(State.Established), "10s");
});
```

`setup()`'s return value — if you return one — is passed to each scenario body
(and to `teardown`) as its first argument, which is handy when a fixture must be
built per scenario. It is typed `any`, so the closure-variable pattern above is
the one that keeps full type-checking.

### Parametrised scenarios: `scenario.each`

To run the same body over a table of inputs, `scenario.each(table)` returns a
registration function: it registers one scenario per row and passes the row to the
body as a second argument. `$key` tokens in the name are replaced with that row's
field, so each scenario gets a distinct — and individually selectable — name:

```js
scenario.each([
  { kind: "internal", target: "201", within: "10s" },
  { kind: "external", target: "+4921112345", within: "20s" },
])("dial $kind target reaches ringing", { tags: ["dialplan"] }, async (ctx, p) => {
  caller.dial(p.target);
  await until(() => expect(caller.state).toBe(State.Ringing), p.within);
});
```

The options object is optional (`scenario.each(table)(name, body)` works too) and
applies to every row. Rows are registered in table order.

## Type-checking a plain `.js` scenario

Generate the definitions once and point a `jsconfig.json` at them; with
`// @ts-check` at the top of each scenario, your editor — and `tsc --noEmit` in
CI — checks the whole DSL:

```sh
ringo-flow definitions --lang js ringo-flow.d.ts
```

```jsonc
// jsconfig.json
{
  "compilerOptions": {
    "checkJs": true,
    "strict": true,
    "noEmit": true,
    "target": "es2022",
    "module": "esnext",
    "types": []
  },
  "files": ["ringo-flow.d.ts", "scenario.js"]
}
```

The `.d.ts` declares the whole DSL as **ambient globals** (`scenario`,
`new Agent(...)`, `expect`, …), so a scenario needs no imports for them. A wrong
matcher, an unknown agent-config key or a forgotten `await` is reported before
baresip ever starts.

## Writing scenarios in TypeScript

Scenarios execute as JavaScript, but you can author them in real TypeScript and
transpile to JS for `ringo-flow run`.

1. Generate the type definitions and point a `tsconfig.json` at them:

   ```sh
   ringo-flow definitions --lang js ringo-flow.d.ts   # writes the .d.ts next to your scenarios
   ```
   ```jsonc
   // tsconfig.json
   {
     "compilerOptions": {
       "strict": true,
       "noEmit": true,
       "target": "es2022",
       "module": "esnext",
       "types": []
     },
     "files": ["ringo-flow.d.ts", "scenario.ts"]
   }
   ```

2. Write `scenario.ts`. The globals and the `Agent`/`MockServer` classes are typed, so
   `tsc` (or your editor) catches a wrong matcher, an unknown config key or a bad
   argument *before* baresip ever starts:

   ```ts
   scenario("answered call", async () => {
     const caller = new Agent("caller", { username: env("A_USER"), domain: env("SIP_DOMAIN") });
     caller.register();
     await until(() => expect(caller.registered).toBeTruthy(), "10s");
   });
   ```

3. Type-check, transpile with Bun, and run the emitted JS:

   ```sh
   tsc --noEmit                                 # optional: fail on a type error
   bun build scenario.ts --outfile scenario.js  # strip types → plain JS
   ringo-flow run scenario.js
   ```

`bun build` strips the type annotations and leaves the ambient globals untouched, so
the runtime executes a plain `.js`. Relative `import`s of helper `.ts` files are bundled
into the one output — or keep them as separate `.js` and let `ringo-flow` resolve the
`import`s at run time.

## Selecting, tagging and skipping

The [`scenario(name, { … }, body)`](js-api/interfaces/ScenarioOptions.md) options
control which scenarios run:

- **Tags** — `{ tags: ["smoke"] }`, then `--tag smoke` / `--exclude-tag slow`.
- **Skip** — `{ skip: true }` or `{ skip: "reason" }` disables a scenario
  statically; or call [`skip("reason")`](js-api/functions/skip.md) at runtime
  (e.g. env-gated).
- **Focus** — `{ only: true }` runs only the focused scenario(s), run-wide.

Skipped scenarios are reported but don't fail the run.

## More

- [Assertions and matchers](js-api/interfaces/Assertion.md) — the full matcher set.
- [Audio testing](audio.md) — send tones/files and assert what's received.
- [HTTP & webhooks](http-and-webhooks.md) — call and mock a backend API.
- [Rhai frontend](rhai.md) — the deprecated frontend and how to migrate off it.
