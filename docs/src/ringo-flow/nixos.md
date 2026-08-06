# NixOS module

The flake ships a `services.ringo-flow` module that runs
[`ringo-flow serve`](monitoring.md) as a hardened systemd service. Point your
NixOS host at the flake and enable it:

```nix
{
  inputs.ringo.url = "github:davidborzek/ringo";

  outputs = { nixpkgs, ringo, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ringo.nixosModules.default
        {
          services.ringo-flow = {
            enable = true;
            listen = "0.0.0.0:9090";
            openFirewall = true;
            monitors.smoke = {
              path = "/var/lib/ringo-flow/smoke.js";
              schedule = "*/5 * * * *";      # cron; omit for on-demand only
            };
          };
        }
      ];
    };
  };
}
```

The module generates the [`monitor.toml`](monitoring.md#configuration) from the
options below and runs the service as a transient `DynamicUser`. `ringo-flow`
writes nothing to disk, so the unit runs with `ProtectSystem=strict` and no state
directory.

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `enable` | `false` | Enable the service. |
| `package` | flake's `ringo-flow` | Package providing the `ringo-flow` binary. |
| `listen` | `"127.0.0.1:9090"` | Bind address for the HTTP API and `/metrics`. |
| `openFirewall` | `false` | Open the TCP port parsed from `listen`. |
| `scheduler` | `true` | Run the cron scheduler for scheduled monitors. |
| `timeout` | `"300s"` | Global per-run timeout (overridable per monitor). |
| `metrics.enable` | `true` | Expose Prometheus `/metrics`. |
| `monitors.<name>` | `{}` | Scenario monitors (see below). |
| `logLevel` | `"info"` | Log level (`RUST_LOG` overrides it). |
| `logFormat` | `"json"` | `text` or `json` (→ journald). |
| `environmentFile` | `null` | systemd `EnvironmentFile` for secrets (see below). |
| `settings` | `{}` | Extra raw keys merged into the generated `monitor.toml`. |
| `configFile` | `null` | Use this `monitor.toml` verbatim instead of the generated one. |
| `extraArgs` | `[]` | Extra arguments appended to `ringo-flow serve`. |

Each `monitors.<name>` entry maps 1:1 to a `[[monitor]]` block:

| Field | Default | Description |
|-------|---------|-------------|
| `path` | — | Scenario file (`.js`, or a deprecated `.rhai`) or a directory of scenarios. |
| `schedule` | `null` | Cron (5- or 6-field); `null` = on-demand only via `POST /run/<name>`. |
| `timeout` | `null` | Per-monitor timeout override. |
| `envFile` | `[]` | dotenv files with SIP credentials (see [Secrets](#secrets)). |
| `scenario` | `null` | Scenario-name filter when `path` is a directory. |
| `tags` | `[]` | Tag filter. |

> When `configFile` is set it takes full control — the `listen`, `monitors`,
> `scheduler`, `timeout` and `metrics` options no longer shape the file (though
> `listen` is still used for `openFirewall`).

## Secrets

`ringo-flow serve` needs two kinds of secret, and **neither belongs in the Nix
store**:

- **SIP credentials** — the scenarios read `SIP_DOMAIN` / `*_USER` / `*_PASS`
  from a monitor's `envFile` (a dotenv file), not from `monitor.toml`.
- **`/metrics` bearer token** — read only from the environment variable
  `RINGO_FLOW_SERVE_METRICS_TOKEN`, injected via `environmentFile`.

Both pair cleanly with [sops-nix](https://github.com/Mic92/sops-nix): the SIP
dotenv stays on tmpfs under `/run/secrets`, and the token is rendered into an
`EnvironmentFile`.

```nix
# SIP credentials as a dotenv secret
sops.secrets."ringo-flow/sip.env" = {
  sopsFile = ./secrets/ringo.yaml;
  format = "dotenv";
};

# Metrics token rendered into a KEY=VALUE EnvironmentFile
sops.secrets."ringo-flow/metrics-token" = { sopsFile = ./secrets/ringo.yaml; };
sops.templates."ringo-flow.env".content = ''
  RINGO_FLOW_SERVE_METRICS_TOKEN=${config.sops.placeholder."ringo-flow/metrics-token"}
'';

services.ringo-flow = {
  enable = true;
  environmentFile = config.sops.templates."ringo-flow.env".path;
  monitors.smoke = {
    path = "/var/lib/ringo-flow/smoke.js";
    schedule = "*/5 * * * *";
    envFile = [ config.sops.secrets."ringo-flow/sip.env".path ];
  };
};
```

sops-nix populates `/run/secrets` during system activation, before the service
starts.

## Hardening

The unit runs unprivileged (`DynamicUser`) with `NoNewPrivileges`,
`ProtectSystem=strict`, `ProtectHome`, `PrivateTmp`/`PrivateDevices`, kernel and
cgroup protections, and a `@system-service` syscall filter. The network stays
open — `serve` makes outbound SIP/RTP and binds the HTTP listener, and baresip's
`netroam` uses `AF_NETLINK`, so `RestrictAddressFamilies` allows
`AF_INET`/`AF_INET6`/`AF_UNIX`/`AF_NETLINK` (do not add `PrivateNetwork`).

See [Monitoring](monitoring.md) for the metrics, HTTP API and Grafana scrape
config.
