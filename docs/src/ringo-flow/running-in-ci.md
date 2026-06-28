# Running in CI

ringo-flow is built to run unattended on a build server: it's headless (virtual
audio), exits non-zero on failure, and can emit machine-readable output.

## Exit code and output

The process exits non-zero if any scenario fails, so a CI step fails naturally.
Add `--json` for one JSON object per event (NDJSON) instead of the human log:

```sh
ringo-flow run scenarios/ --json
```

Other handy flags: `-q` (only failures + result), `-v` (show every assertion),
`--log` (write the SIP signaling to stderr, or `--log <file>`), `--save-audio`
(dump sent/received WAVs), `--no-color`.

## Metrics

Add `--metrics` to emit a per-agent media-quality summary at each scenario's
end. On its own it prints a compact human line; combined with `--json` it adds a
`metric` event to the NDJSON stream, ready to scrape:

```sh
ringo-flow run scenarios/ --json --metrics
```

```json
{"event":"metric","scenario":"call quality","agent":"caller","registered":true,"mos":4.24,"jitter_ms":2.1,"packet_loss_pct":0.0,"rtt_ms":18.0,"rx_lost":0,"ts":"…"}
```

The quality fields ([MOS, jitter, loss, RTT](call-quality.md)) are present only
when the agent had a measurable call; `registered` is always emitted. Without
`--metrics` the stream is unchanged (no extra stat reads).

## Credentials and environment

Scenarios read secrets via [`env(...)`](api/environment.md#env). Provide them as
environment variables, or from a dotenv file:

```sh
ringo-flow run scenarios/ --env-file ci.env
```

A sibling `<scenario>.env` next to a file is layered on top automatically. Keep
real credentials in your CI secret store, not in the repo.

## Selecting what to run

Run a whole directory (all `*.rhai`, recursively) or a subset:

```sh
ringo-flow run scenarios/                       # everything
ringo-flow run scenarios/ --scenario "answered" # by name (re: for regex)
ringo-flow run scenarios/ --tag smoke           # by tag
ringo-flow run scenarios/ --exclude-tag slow    # drop tagged ones
```

See [Writing scenarios](writing-scenarios.md) for tags, `skip` and `only`.

## Docker

A small image with baresip compiled in is published to GHCR on each release —
nothing to install:

```sh
docker run --rm --network host \
  -e SIP_DOMAIN=example.com -e A_USER=alice -e A_PASS=… -e B_USER=bob -e B_PASS=… \
  -v "$PWD/scenarios:/scn:ro" \
  ghcr.io/davidborzek/ringo-flow:latest run /scn
```

`--network host` is the simplest way to get working SIP/RTP and DNS. Use
`:latest` or pin `:<version>`. See the
[README](https://github.com/davidborzek/ringo/tree/main/crates/ringo-flow#docker)
for recordings, dotenv mounting and private-CA TLS.
