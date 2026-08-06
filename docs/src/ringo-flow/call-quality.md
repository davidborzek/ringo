# Call quality

Beyond *was there audio* ([Audio testing](audio.md)), ringo-flow can assert on
*how good* the audio was — the RTP media metrics each agent reports for its call,
grouped under [`agent.quality`](js-api/classes/Agent.md#quality):

| Field | Meaning | Unit |
|-------|---------|------|
| `agent.quality.mos` | Estimated Mean Opinion Score | 1.0 (bad) – 4.5 (excellent) |
| `agent.quality.rtt` | Round-trip time | milliseconds |
| `agent.quality.jitter` | Receive-side inter-arrival jitter | milliseconds |
| `agent.quality.packetLoss` | Receive-side packet loss | percent |

The **MOS** is an estimate from the simplified ITU-T G.107 E-model, derived from
latency, jitter and loss — a single number to gate call quality on.

## When the values are available

The metrics come from **RTCP reports**, which the peers exchange only about
**every ~5 seconds**. So right after the call is established `agent.quality` is
still `undefined` — let the call run a few seconds first:

```js
await until(() => expect(caller.quality).toBeDefined(), "10s");
```

The whole object appears at once (the fields arrive together, so there are no
per-field gaps), which is why the optional chain in `caller.quality?.mos` is only
needed before the first report.

The values are **snapshotted when the call closes**, so they survive the hangup
— you can read or assert on them **after** the call, not just during it.

## Example

```js
// @ts-check
/** @type {Agent} */ let caller;
/** @type {Agent} */ let callee;

setup(() => {
  caller = new Agent("caller", { username: env("A_USER"), domain: env("SIP_DOMAIN"), password: env("A_PASS") });
  callee = new Agent("callee", { username: env("B_USER"), domain: env("SIP_DOMAIN"), password: env("B_PASS") });
});

scenario("call quality", async () => {
  caller.dial(callee);
  await until(() => expect(callee.state).toBe(State.Ringing));
  callee.accept();
  await until(() => expect(caller.state).toBe(State.Established));

  // Let RTCP accumulate, then wait for the first report:
  const q = await until(() => expect(caller.quality).toBeDefined().value(), "10s");

  log(`caller → MOS ${q.mos} · RTT ${q.rtt}ms · jitter ${q.jitter}ms · loss ${q.packetLoss}%`);

  caller.hangup();
  await until(() => expect(caller.state).toBe(State.Idle));

  // The snapshot survives the hangup — assert on the final values:
  expect(caller.quality?.mos).toBeGreaterThanOrEqual(4.0);
  expect(caller.quality?.packetLoss).toBeLessThanOrEqual(1.0);
  expect(caller.quality?.rtt).toBeLessThanOrEqual(150);
});
```

> The values are raw floats (e.g. `MOS 4.236…`). To shorten a log line, round
> them: `` log(`MOS ${q.mos.toFixed(2)}`) ``.

## Exporting metrics

To record these values without writing assertions — e.g. for trend monitoring —
run with [`--metrics`](running-in-ci.md#metrics). ringo-flow then emits a per-agent
`metric` event (MOS, jitter, loss, RTT + `registered`) at each scenario's end,
which a machine consumer can scrape from the `--json` stream.
