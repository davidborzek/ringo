# Rhai frontend (deprecated)

> **The Rhai frontend is deprecated and will be removed in a future release.**
> Write new scenarios in JavaScript or TypeScript — see
> [Writing scenarios](writing-scenarios.md). Existing `.rhai` scenarios keep
> running for now, and the [Rhai API reference](api/index.md) stays published
> until the frontend is removed.

ringo-flow started out with [Rhai](https://rhai.rs) as its scripting language. It
did the job, but writing real test suites in it turned out to be worse than
writing them in JavaScript in three concrete ways.

## Why it is going away

**No usable IDE tooling.** This is the big one. A Rhai language server does exist
— [rhaiscript/lsp](https://github.com/rhaiscript/lsp) — but its own README calls
it *"experimental … incomplete and not recommended for general use"*, it has never
cut a release, and its last commit landed in October 2022. No editor plugin ships
it either: the VS Code extension lives inside that repo and has to be built and
side-loaded by hand. Getting anything beyond syntax highlighting is a
build-it-yourself exercise, and what you end up with is a stale experiment.

In practice that means no completion, no hover, no jump-to-definition and — most
importantly — no errors until you actually run the script. A typo in a config key
or a matcher name surfaces after baresip has started and the SIP traffic has
begun. The shipped [`.d.rhai`](ringo-flow.d.rhai) documents the signatures, but
nothing checks your script against them.

The JS frontend ships a generated [`ringo-flow.d.ts`](ringo-flow.d.ts) instead.
Every editor with TypeScript support — that is, essentially all of them —
type-checks the whole DSL: agent config keys, matcher names, argument types and
`await`-ing the blocking verbs. With `// @ts-check` in a plain `.js` file, or by
authoring in `.ts`, mistakes surface while you type instead of mid-call.

**Missing language features.** Rhai's scoping rules bite as soon as a suite grows
past one file's worth of top-level code. Most painfully, a `fn` cannot see
top-level variables — Rhai functions do not close over the enclosing scope, so
shared fixtures have to be threaded through the scenario context or re-created in
every helper. Neither is there `async`/`await`, so concurrent waiting needs a
dedicated `parallel(...)` verb rather than the language's own primitives.
JavaScript has closures, real modules, `async`/`await`, destructuring, template
literals, `JSON` and a standard library — none of which had to be invented for
the DSL.

**Hardly anyone knows it.** Rhai is a niche embedded scripting language, so
everyone touching a scenario has to learn a new syntax (`#{ … }` maps, `||`
closures, `State::Ringing` paths) before writing the first test. JavaScript is
already familiar to nearly everyone who would write an integration test, and it
is what test runners in the wider ecosystem look like — `expect(...)`,
Jest-style matchers, `scenario.each` tables.

## Migrating a scenario

The vocabulary maps almost one-to-one; the differences are naming
(`snake_case` → `camelCase`), object literals, and `await` on the blocking verbs.

| Rhai | JavaScript |
|------|------------|
| `agent("A", #{ … })` | `new Agent("A", { … })` |
| `#{ key: value }` | `{ key: value }` |
| `\|\| assert(x).equals(y)` | `() => expect(x).toBe(y)` |
| `await_until(cond, "10s")` | `await until(cond, "10s")` |
| `default_timeout("10s")` | `defaultTimeout("10s")` |
| `State::Ringing` | `State.Ringing` |
| `assert(x).is_true()` | `expect(x).toBeTruthy()` |
| `assert(x).is_present()` | `expect(x).toBeDefined()` |
| `assert(x).at_least(n)` | `expect(x).toBeGreaterThanOrEqual(n)` |
| `assert(x).at_most(n)` | `expect(x).toBeLessThanOrEqual(n)` |
| `assert(x).contains(s)` | `expect(x).toContain(s)` |
| `a.send_audio(silent())` | `a.sendAudio(silence())` |
| `a.verify_audio(440, "5s")` | `await a.verifyAudio(440, "5s")` |
| `verify_audio_connection(a, b)` | `await verifyAudioConnection(a, b)` |
| `agent.quality.packet_loss` | `agent.quality?.packetLoss` |
| `mock_server()` | `new MockServer()` |
| `hooks.on(m, p, \|req\| …)` | `hooks.respond(m, p, (req) => …)` |
| `hooks.request_count(p)` | `hooks.requestCount(p)` |
| `hooks.last_request(p)` | `hooks.lastRequest(p)` |
| `req.json("event")` | `JSON.parse(req.body).event` |
| `parallel([\|\| …, \|\| …])` | `await Promise.all([…])` |
| `load_env("ci.env")` | `loadEnv("ci.env")` |

Two structural differences beyond the names:

- **`await` the blocking verbs.** `until`, `wait`, `http`, `verifyAudio` and
  `verifyAudioConnection` return Promises. Instant verbs (`dial`, `accept`,
  `hangup`, `dtmf`, …) stay synchronous. A scenario body that awaits anything
  must be `async`.
- **Fixtures live in closure variables.** Rhai forced shared state through the
  `setup()` context because functions could not see the top level. In JS you can
  still use the `ctx` argument, but declaring `let caller;` up top and assigning
  it in `setup()` is both shorter and fully typed — see
  [Writing scenarios](writing-scenarios.md).

The full side-by-side is in the two API references:
[JS](js-api/index.md) and [Rhai](api/index.md).

## Running Rhai in the meantime

Nothing changes for existing scenarios. The frontend is picked from the file
extension, so `.rhai` files keep using Rhai and `.js` files use the JS frontend:

```sh
ringo-flow run legacy.rhai      # Rhai (deprecated)
ringo-flow run scenario.js      # JavaScript
```

`--lang rhai` forces the Rhai frontend explicitly, and `ringo-flow definitions`
still writes the `.d.rhai`. A directory containing any `.js` file selects the JS
frontend, so migrate a suite file by file rather than mixing both in one run.
