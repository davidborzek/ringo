# Home-Manager module: programs.ringo — installs ringo-phone and manages its
# config declaratively.
#
# ringo-phone has two config layers with different mutability:
#
#   * ~/.config/ringo/ringo.toml  — global (theme/picker/baresip/hooks). ringo
#     only ever READS it, so it is managed as a read-only store symlink.
#   * ~/.config/ringo/profiles/<name>/profile.toml — per SIP profile. The TUI
#     REWRITES these at runtime (add/clone/edit/rename, in-call edit), so they
#     must be real writable files, not store symlinks (a write to a read-only
#     store target would fail). They are rendered at activation instead.
#
# Secrets: prefer `passwordFile` / `passwordCommand`, which render ringo's native
# `password_file` / `password_cmd` keys — the phone resolves the password at
# launch, so no secret is ever written into profile.toml or the Nix store. The
# inline `password` is a discouraged fallback (it lands in the world-readable
# store via the rendered TOML).
{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.ringo;
  tomlFormat = pkgs.formats.toml { };
  system = pkgs.stdenv.hostPlatform.system;
  defaultPackage = self.packages.${system}.ringo-phone or null;

  profileType = lib.types.submodule {
    options = {
      settings = lib.mkOption {
        type = tomlFormat.type;
        default = { };
        example = lib.literalExpression ''
          {
            username = "alice";
            domain = "sip.example.com";
            regint = 600;
            audio_codecs = [ "opus" "PCMU" ];
          }
        '';
        description = ''
          Contents of this profile's profile.toml, minus the password (set via
          {option}`passwordFile`/{option}`passwordCommand`/{option}`password`).
          Must at least define `username` and `domain`. Fields map 1:1 to ringo's
          profile schema.
        '';
      };
      passwordFile = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "/run/user/1000/secrets/ringo-alice";
        description = ''
          Path written to ringo's native `password_file` key. ringo reads the
          password from this file at launch — the secret never enters
          profile.toml or the Nix store. Point it at a runtime secret (e.g. a
          sops-nix secret path).
        '';
      };
      passwordCommand = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "pass show sip/alice";
        description = ''
          Command written to ringo's native `password_cmd` key. ringo runs it
          (via `sh -c`) at launch and uses its stdout as the password. Do not
          embed the secret literally here (that would land in the store) — call
          a secret manager.
        '';
      };
      password = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = ''
          Inline SIP password. **Discouraged** — it is rendered into profile.toml
          and thus into the world-readable Nix store. Prefer
          {option}`passwordFile` or {option}`passwordCommand`.
        '';
      };
      mutable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = ''
          Whether the TUI owns this profile after it is first created.

          - `false` (default): Nix is authoritative. The profile is rewritten on
            every `home-manager switch`, so TUI edits to it are reverted on the
            next switch. Keeps the profile reproducible.
          - `true`: seed once. The profile is written only if it does not exist
            yet, then left to the TUI. Later changes to {option}`settings` etc.
            are NOT propagated (delete the file or flip this to `false` once to
            re-apply) — same tradeoff as `users.mutableUsers`.

          Profiles not declared here are never touched, regardless of this flag.
        '';
      };
    };
  };

  # profile.toml body: the user settings plus whichever native password key is
  # configured. Secrets in passwordFile/passwordCommand are references, not the
  # secret itself, so the whole file is safe to render to the store.
  profileBody =
    p:
    p.settings
    // lib.optionalAttrs (p.passwordFile != null) { password_file = p.passwordFile; }
    // lib.optionalAttrs (p.passwordCommand != null) { password_cmd = p.passwordCommand; }
    // lib.optionalAttrs (p.password != null) { password = p.password; };

  mkProfileActivation =
    name: p:
    let
      file = tomlFormat.generate "ringo-profile-${name}.toml" (profileBody p);
      # A mutable profile is seeded once, then left to the TUI; an immutable
      # (default) profile is rewritten on every activation.
      seedGuard = lib.optionalString p.mutable ''[ -e "$_dst" ] && _write=0'';
    in
    ''
      _dir="$HOME/.config/ringo/profiles/${name}"
      _dst="$_dir/profile.toml"
      _write=1
      ${seedGuard}
      if [ "$_write" = 1 ]; then
        run mkdir -p "$_dir"
        run install -m600 ${file} "$_dst"
      fi
    '';
in
{
  options.programs.ringo = {
    enable = lib.mkEnableOption "ringo-phone, the terminal SIP softphone";

    package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = defaultPackage;
      defaultText = lib.literalExpression "ringo.packages.\${system}.ringo-phone";
      description = "The ringo-phone package providing the `ringo` binary.";
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      example = lib.literalExpression ''
        {
          theme = "tokyo-night";
          baresip.sip_cafile = "/etc/ssl/certs/ca-bundle.crt";
        }
      '';
      description = ''
        Global ~/.config/ringo/ringo.toml (theme/picker/baresip/hooks). ringo
        only reads this file, so it is managed as a read-only store symlink.
      '';
    };

    profiles = lib.mkOption {
      type = lib.types.attrsOf profileType;
      default = { };
      description = "Declarative SIP profiles, keyed by profile name.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.package != null;
        message = "programs.ringo.package is null: no ringo-phone package for system ${system}. Set programs.ringo.package explicitly.";
      }
    ]
    ++ lib.mapAttrsToList (name: p: {
      assertion =
        lib.count (x: x != null) [
          p.password
          p.passwordFile
          p.passwordCommand
        ] <= 1;
      message = "programs.ringo.profiles.${name}: set at most one of `password`, `passwordFile`, `passwordCommand`.";
    }) cfg.profiles;

    home.packages = [ cfg.package ];

    # ringo hardcodes ~/.config/ringo (it ignores XDG_CONFIG_HOME), so target
    # the literal path via home.file rather than xdg.configFile.
    home.file.".config/ringo/ringo.toml" = lib.mkIf (cfg.settings != { }) {
      source = tomlFormat.generate "ringo.toml" cfg.settings;
    };

    home.activation = lib.mkIf (cfg.profiles != { }) {
      ringoProfiles = lib.hm.dag.entryAfter [ "writeBoundary" ] (
        lib.concatStringsSep "\n" (lib.mapAttrsToList mkProfileActivation cfg.profiles)
      );
    };
  };
}
