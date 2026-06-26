# Call state

`agent.state` returns a **`CallState`** — a call's current phase. Compare it against the `State::*` constants, usually inside `await_until`:

```rust
await_until(|| assert(callee.state).equals(State::Ringing));
```

- `State::Idle` — No active call.
- `State::Ringing` — A call is ringing — incoming or outgoing — but not yet answered.
- `State::Established` — The call is connected and media is flowing.

