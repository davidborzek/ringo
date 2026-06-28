# Monitoring

`ringo-flow serve` turns your scenarios into a **synthetic monitor**: it runs
them on a schedule (and on demand over HTTP) and exposes the results as
[Prometheus](https://prometheus.io/) metrics — call success, MOS, jitter, loss,
RTT and registration, per scenario and agent. Point Grafana at it and you have a
live view of how your telephony actually behaves.

```sh
ringo-flow serve monitor.toml
```

## Configuration

The monitor reads a `monitor.toml`:

```toml
# HTTP listen address (default 127.0.0.1:9090).
listen = "0.0.0.0:9090"
# Default per-run timeout, overridable per scenario.
timeout = "120s"
# The ringo-flow binary spawned per run. Defaults to the running executable,
# so a single binary both serves and runs — only set this to use another build.
# binary = "/usr/local/bin/ringo-flow"

# Prometheus /metrics endpoint (optional; enabled by default).
[metrics]
enabled = true              # set false to not expose /metrics at all (404)
# bearer_token = "s3cret"   # if set, /metrics requires Authorization: Bearer s3cret

# A monitor names a scenario file (which may hold a whole suite) plus a schedule.
[[monitor]]
name = "smoke"                 # unique — the metric label and /run/<name>
path = "scenarios/smoke.rhai"  # a file or a directory of *.rhai
schedule = "*/5 * * * *"       # cron (5- or 6-field); omit for on-demand only
env_file = ["ci.env"]          # optional --env-file(s)

[[monitor]]
name = "quality"
path = "scenarios/quality.rhai"
schedule = "0 * * * *"         # hourly
timeout = "180s"               # per-monitor override
scenario = "answered"          # optional --scenario name filter within the file
tags = ["smoke"]               # optional --tag filters
```

A monitor with no `schedule` is only reachable via `POST /run/<name>` — handy
for ad-hoc checks or driving runs from an external scheduler.

### Overriding the basics (flags / env)

The deployment basics can be overridden without editing the config — useful in
containers. Precedence is **flag > env > config file**:

| Flag | Env | Overrides |
|------|-----|-----------|
| `--listen <host:port>` | `RINGO_FLOW_SERVE_LISTEN` | full listen address |
| `--port <port>` | `RINGO_FLOW_SERVE_PORT` | listen port only (keeps host; wins over `--listen`) |
| `--timeout <dur>` | `RINGO_FLOW_SERVE_TIMEOUT` | default per-run timeout |
| `--metrics <true\|false>` | `RINGO_FLOW_SERVE_METRICS` | `/metrics` on/off |
| `--binary <path>` | `RINGO_FLOW_SERVE_BINARY` | the spawned ringo-flow binary |
| — | `RINGO_FLOW_SERVE_METRICS_TOKEN` | `/metrics` bearer token (env only — kept out of the process args) |

```sh
RINGO_FLOW_SERVE_PORT=8080 RINGO_FLOW_SERVE_METRICS_TOKEN=s3cret \
  ringo-flow serve monitor.toml
```

## How it runs

Each run is a fresh `ringo-flow run --json --metrics` **subprocess**. That's
deliberate: the baresip backend initialises global state once per process, so a
long-lived server can't reuse it — and a subprocess also gives crash isolation
(a backend crash can't take the monitor down) and a hard per-run timeout.

Runs are **serialised** through a single worker — one backend per process means
two runs at once would collide. Both the cron schedulers and `POST /run` feed
that one queue, so a manual trigger waits behind an in-flight run.

## HTTP API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/metrics` | GET | Prometheus exposition (scrape this) |
| `/monitors` | GET | The configured monitors as JSON |
| `/run/{name}` | POST | Run a monitor now; waits and returns the result (200 pass / 502 fail / 404 unknown). `?async=true` enqueues and returns **202** immediately without waiting (result lands in `/metrics`). |
| `/healthz` | GET | Liveness — always `ok` |

A synchronous run's result is grouped by the scenarios the file executed, each
with its agents:

```sh
curl -X POST http://localhost:9090/run/smoke            # waits for the result
curl -X POST "http://localhost:9090/run/smoke?async=true"  # 202, returns at once
```
```json
{
  "monitor": "smoke",
  "passed": true,
  "timed_out": false,
  "duration_ms": 4120,
  "scenarios": [
    { "name": "callee accepts", "passed": true,
      "agents": [ { "agent": "Caller", "registered": true, "mos": 4.39, "jitter_ms": 8.2, "packet_loss_pct": 0.0, "rtt_ms": 31.8 } ] }
  ]
}
```

## Metrics

Metrics are labelled by `monitor` (the configured `[[monitor]]`) → `scenario`
(a scenario inside the file) → `agent`.

| Metric | Type | Labels | Meaning |
|--------|------|--------|---------|
| `ringo_monitor_runs_total` | counter | `monitor`, `result` | Runs by result (`pass`/`fail`/`timeout`) |
| `ringo_monitor_last_success` | gauge | `monitor` | 1 if the last run passed, else 0 |
| `ringo_monitor_last_duration_seconds` | gauge | `monitor` | Duration of the last run |
| `ringo_monitor_last_run_timestamp_seconds` | gauge | `monitor` | Unix time of the last run |
| `ringo_scenario_last_success` | gauge | `monitor`, `scenario` | 1 if that scenario passed in the last run |
| `ringo_agent_registered` | gauge | `monitor`, `scenario`, `agent` | 1 if the agent was registered |
| `ringo_call_mos` | gauge | `monitor`, `scenario`, `agent` | [MOS](call-quality.md) of the last call |
| `ringo_call_jitter_ms` | gauge | `monitor`, `scenario`, `agent` | Jitter, milliseconds |
| `ringo_call_packet_loss_pct` | gauge | `monitor`, `scenario`, `agent` | Packet loss, percent |
| `ringo_call_rtt_ms` | gauge | `monitor`, `scenario`, `agent` | Round-trip time, milliseconds |

The `ringo_call_*` gauges come from the run's [`metric`
events](running-in-ci.md#metrics); a field is omitted for an agent that had no
measurable call.

A Prometheus scrape config:

```yaml
scrape_configs:
  - job_name: ringo-flow
    static_configs:
      - targets: ["localhost:9090"]
    # only if [metrics].bearer_token is set:
    # authorization: { credentials: "s3cret" }
```

> Keep the scrape interval shorter than your run cadence — the gauges hold the
> *last* run's values, with no persistence across restarts.

### Disabling / protecting `/metrics`

By default `/metrics` is open (fine when bound to localhost or a trusted
network). The `[metrics]` table changes that:

- `enabled = false` — don't expose `/metrics` at all (returns 404).
- `bearer_token = "…"` — require `Authorization: Bearer …`; requests without it
  get 401.

## Building without the server

`serve` lives behind the `server` feature, which is **on by default**. To build a
smaller binary without it (and without the `toml`/`croner` dependencies):

```sh
cargo build -p ringo-flow --no-default-features
```
