# NixOS module: services.ringo-flow — runs `ringo-flow serve` (the telephony
# scenario monitor: cron-scheduled + on-demand runs, Prometheus /metrics) as a
# hardened systemd service.
#
# Instantiated from the flake with the flake's own `self` so the default
# package resolves to this repo's build.
{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.ringo-flow;
  tomlFormat = pkgs.formats.toml { };
  system = pkgs.stdenv.hostPlatform.system;
  defaultPackage = self.packages.${system}.ringo-flow or null;

  monitorType = lib.types.submodule {
    options = {
      path = lib.mkOption {
        type = lib.types.str;
        example = "/var/lib/ringo-flow/scenarios/register.rhai";
        description = "Path to the .rhai scenario file or a directory of scenarios.";
      };
      schedule = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "*/5 * * * *";
        description = "Cron expression (5- or 6-field) for scheduled runs. Null = on-demand only (via POST /run/<name>).";
      };
      timeout = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "120s";
        description = "Per-monitor run timeout, overriding the global {option}`services.ringo-flow.timeout`.";
      };
      envFile = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        example = [ "/run/secrets/ringo-flow-sip.env" ];
        description = ''
          dotenv files holding this monitor's SIP credentials
          (`SIP_DOMAIN` / `SIP_USER` / `SIP_PASS`, referenced by the scenario).
          Use runtime paths such as sops-nix secrets — these are read by the
          service at runtime and are never copied into the Nix store.
        '';
      };
      scenario = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Named scenario to select when `path` is a directory.";
      };
      tags = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [ ];
        description = "Scenario tag filter passed to the run.";
      };
    };
  };

  # monitor.toml assembled from the module options (skipped when the user
  # supplies their own configFile).
  generatedConfig =
    {
      listen = cfg.listen;
      scheduler = cfg.scheduler;
      timeout = cfg.timeout;
      metrics = {
        enabled = cfg.metrics.enable;
      };
      monitor = lib.mapAttrsToList (
        name: m:
        {
          inherit name;
          inherit (m) path;
        }
        // lib.optionalAttrs (m.schedule != null) { inherit (m) schedule; }
        // lib.optionalAttrs (m.timeout != null) { inherit (m) timeout; }
        // lib.optionalAttrs (m.envFile != [ ]) { env_file = m.envFile; }
        // lib.optionalAttrs (m.scenario != null) { inherit (m) scenario; }
        // lib.optionalAttrs (m.tags != [ ]) { inherit (m) tags; }
      ) cfg.monitors;
    }
    // cfg.settings;

  configFile =
    if cfg.configFile != null then
      cfg.configFile
    else
      tomlFormat.generate "ringo-flow-monitor.toml" generatedConfig;

  # `listen` is "host:port"; take the last colon-separated field as the port
  # for the optional firewall opening (bracketed IPv6 hosts are not parsed —
  # open the port manually in that case).
  listenPort = lib.toInt (lib.last (lib.splitString ":" cfg.listen));
in
{
  options.services.ringo-flow = {
    enable = lib.mkEnableOption "ringo-flow serve, the telephony scenario monitor";

    package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = defaultPackage;
      defaultText = lib.literalExpression "ringo.packages.\${system}.ringo-flow";
      description = "The ringo-flow package providing the `ringo-flow` binary.";
    };

    listen = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1:9090";
      description = "Address the HTTP API and `/metrics` endpoint bind to.";
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Open the TCP port derived from {option}`listen` in the firewall.";
    };

    scheduler = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Run the cron scheduler for monitors that define a `schedule`.";
    };

    timeout = lib.mkOption {
      type = lib.types.str;
      default = "300s";
      description = "Global per-run timeout (overridable per monitor).";
    };

    metrics.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Expose Prometheus metrics at `/metrics`.";
    };

    monitors = lib.mkOption {
      type = lib.types.attrsOf monitorType;
      default = { };
      description = "Scenario monitors, keyed by name (the metric label and `/run/<name>` id).";
      example = lib.literalExpression ''
        {
          register = {
            path = "/var/lib/ringo-flow/register.rhai";
            schedule = "*/5 * * * *";
            envFile = [ "/run/secrets/ringo-flow-sip.env" ];
          };
        }
      '';
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      description = "Extra raw keys merged into the generated monitor.toml (escape hatch, wins over the derived keys).";
    };

    configFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Use this monitor.toml verbatim instead of the generated one. When set,
        the {option}`listen`, {option}`monitors`, {option}`scheduler`,
        {option}`timeout` and {option}`metrics` options no longer shape the
        config file (but {option}`listen` is still used for {option}`openFirewall`).
      '';
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      example = "/run/secrets/ringo-flow.env";
      description = ''
        systemd `EnvironmentFile` with secrets, most notably
        `RINGO_FLOW_SERVE_METRICS_TOKEN=<token>` to protect `/metrics`. Keeps
        the token out of the Nix store and off the command line. Pairs well
        with a sops-nix rendered template.
      '';
    };

    logLevel = lib.mkOption {
      type = lib.types.str;
      default = "info";
      description = "Log level (overridden by `RUST_LOG` if set in the environment).";
    };

    logFormat = lib.mkOption {
      type = lib.types.enum [
        "text"
        "json"
      ];
      default = "json";
      description = "Log format on stderr → journald. `json` gives structured logs.";
    };

    extraArgs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = "Extra arguments appended to `ringo-flow serve`.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.package != null;
        message = "services.ringo-flow.package is null: no ringo-flow package for system ${system}. Set services.ringo-flow.package explicitly.";
      }
    ];

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ listenPort ];

    systemd.services.ringo-flow = {
      description = "ringo-flow telephony scenario monitor";
      documentation = [ "https://github.com/davidborzek/ringo" ];
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];

      serviceConfig = {
        ExecStart = lib.escapeShellArgs (
          [
            "${cfg.package}/bin/ringo-flow"
            "serve"
            (toString configFile)
            "--log-level"
            cfg.logLevel
            "--log-format"
            cfg.logFormat
          ]
          ++ cfg.extraArgs
        );

        EnvironmentFile = lib.mkIf (cfg.environmentFile != null) [ cfg.environmentFile ];

        Restart = "on-failure";
        RestartSec = 5;
        # serve has no graceful shutdown; give in-flight child runs time to be
        # killed (kill_on_drop) before SIGKILL.
        TimeoutStopSec = 30;

        # Run as a transient unprivileged user; serve writes nothing to disk.
        DynamicUser = true;

        # Hardening. serve makes outbound SIP/RTP and binds the HTTP listener,
        # so the network stays open; baresip's netroam uses AF_NETLINK.
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        RestrictSUIDSGID = true;
        RestrictRealtime = true;
        RestrictNamespaces = true;
        LockPersonality = true;
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
          "AF_NETLINK"
        ];
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
          "~@resources"
        ];
      };
    };
  };
}
