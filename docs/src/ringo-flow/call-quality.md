# Call quality

Beyond *was there audio* ([Audio testing](audio.md)), ringo-flow can assert on
*how good* the audio was — the RTP media metrics each agent reports for its call:

| Getter | Meaning | Unit |
|--------|---------|------|
| `agent.mos` | Estimated Mean Opinion Score | 1.0 (bad) – 4.5 (excellent) |
| `agent.rtt` | Round-trip time | milliseconds |
| `agent.jitter` | Receive-side inter-arrival jitter | milliseconds |
| `agent.packet_loss` | Receive-side packet loss | percent |

The **MOS** is an estimate from the simplified ITU-T G.107 E-model, derived from
latency, jitter and loss — a single number to gate call quality on.

## When the values are available

The metrics come from **RTCP reports**, which the peers exchange only about
**every ~5 seconds**. So right after the call is established the getters return
`()` (not present) — you have to let the call run a few seconds first:

```rhai
await_until(|| assert(caller.mos).is_present(), "10s");
```

The values are **snapshotted when the call closes**, so they survive the hangup
— you can read or assert on them **after** the call, not just during it.

## Example

```rhai
scenario("call quality", |ctx| {
    ctx.caller.dial(ctx.callee);
    await_until(|| assert(ctx.callee.state).equals(State::Ringing));
    ctx.callee.accept();
    await_until(|| assert(ctx.caller.state).equals(State::Established));

    // Let RTCP accumulate, then wait for the first report:
    await_until(|| assert(ctx.caller.mos).is_present(), "10s");

    log(`caller → MOS ${ctx.caller.mos} · RTT ${ctx.caller.rtt}ms · ` +
        `jitter ${ctx.caller.jitter}ms · loss ${ctx.caller.packet_loss}%`);

    ctx.caller.hangup();
    await_until(|| assert(ctx.caller.state).equals(State::Idle));

    // The snapshot survives the hangup — assert on the final values:
    assert(ctx.caller.mos).at_least(4.0);
    assert(ctx.caller.packet_loss).at_most(1.0);
    assert(ctx.caller.rtt).at_most(150);
});
```

> The values are raw floats (e.g. `MOS 4.236…`). To shorten a log line, round in
> Rhai: `let mos = (caller.mos * 100.0).round() / 100.0;`.
