# Flow and timing

<a id="await_until"></a>

## await_until(body: Fn)

**Returns** `any`

Re-run the expression until its assertion holds or the default timeout
elapses: `await_until(|| assert(a.registered).is_true())`. Returns the
body's value, so `.value()` can bind a verified value.

<a id="await_until"></a>

## await_until(body: Fn, within: string)

**Returns** `any`

Like `await_until(body)` but with an explicit timeout, e.g. `"15s"`.

**Example**

```rust
await_until(|| assert(b.state).equals(State::Ringing), "15s");
```

<a id="default_timeout"></a>

## default_timeout(duration: string)

Set the default `await_until` timeout for the rest of the script (e.g. `"10s"`).

<a id="parallel"></a>

## parallel(tasks: array)

**Returns** `array`

Run the given zero-arg closures concurrently and wait for all; returns
their results as an array, and fails if any task fails. Use it for
independent blocking work, e.g. `verify_audio` on several agents at once.
Tasks may share captured variables (each gets an independent snapshot,
so they can't race). Don't overlap `await_until` across tasks; its
silencing is global.

<a id="wait"></a>

## wait(seconds: int)

Hold for N seconds; FAILS if a call that is established at the start drops.

**Example**

```rust
wait(3); // the call must stay up for 3s
```

