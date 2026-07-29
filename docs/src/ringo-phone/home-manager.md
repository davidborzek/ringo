# Home-Manager module

The flake ships a `programs.ringo` Home-Manager module that installs ringo and
manages its configuration declaratively. Import it and enable:

```nix
{
  inputs.ringo.url = "github:davidborzek/ringo";

  # in your Home-Manager configuration:
  imports = [ ringo.homeManagerModules.default ];

  programs.ringo = {
    enable = true;
    settings.theme = "tokyo-night";
    profiles.work = {
      settings = {
        username = "alice";
        domain = "sip.example.com";
        regint = 600;
        audio_codecs = [ "opus" "PCMU" ];
      };
      passwordFile = config.sops.secrets."ringo/work".path;
    };
  };
}
```

## Two config layers, two mutabilities

ringo has two kinds of config file, and they need different handling under Nix:

| File | Rewritten by the TUI? | How the module manages it |
|------|-----------------------|---------------------------|
| `~/.config/ringo/ringo.toml` (theme, picker, baresip, hooks) | **No** — ringo only reads it | Read-only symlink into the store, from `settings`. |
| `~/.config/ringo/profiles/<name>/profile.toml` (SIP account) | **Yes** — add/clone/edit/rename and in-call edit all call `save()` | Rendered at activation as a real, writable `0600` file (a store symlink would be read-only and break the TUI's write). |

Because the TUI writes profiles back, a declaratively-managed profile and the
TUI are two writers of the same file. Each profile's `mutable` flag picks who
wins (mirroring `users.mutableUsers`):

- `mutable = false` (default) — Nix is the source of truth. The profile is
  rewritten on every `home-manager switch`; TUI edits to it are reverted on the
  next switch. Keeps it reproducible.
- `mutable = true` — the profile is written only if it doesn't exist yet, then
  left to the TUI. Later changes to `settings`/`passwordFile` in Nix are NOT
  propagated (delete the file, or flip to `false` once, to re-apply).

Profiles you don't declare are never touched, so you can always create ad-hoc
profiles in the TUI alongside the managed ones.

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `enable` | `false` | Install ringo and manage its config. |
| `package` | flake's `ringo-phone` | Package providing the `ringo` binary. |
| `settings` | `{}` | Contents of `~/.config/ringo/ringo.toml`. See [Configuration](configuration.md). |
| `profiles.<name>` | `{}` | Declarative SIP profiles (see below). |

Each `profiles.<name>`:

| Field | Default | Description |
|-------|---------|-------------|
| `settings` | `{}` | The profile's fields, **minus the password** (at least `username` and `domain`). Maps 1:1 to the [profile schema](profiles.md). |
| `passwordFile` | `null` | Path written to ringo's native `password_file`. ringo reads the password from it at launch — the secret never enters profile.toml or the store. Point it at a runtime secret (e.g. a sops-nix secret path). |
| `passwordCommand` | `null` | Command written to ringo's native `password_cmd`; ringo runs it at launch and uses its stdout. Call a secret manager — don't embed the secret literally. |
| `password` | `null` | Inline plaintext. **Discouraged** — rendered into profile.toml and thus into the world-readable store. Prefer `passwordFile`/`passwordCommand`. |
| `mutable` | `false` | If `true`, seed the profile once then leave it to the TUI; if `false`, Nix rewrites it on every switch (see above). |

Set at most one of `passwordFile` / `passwordCommand` / `password` per profile.

## Secrets

Prefer `passwordFile` or `passwordCommand`: they render ringo's native
`password_file` / `password_cmd` keys, so ringo resolves the password at launch
and **no secret is ever written into profile.toml or the Nix store**. With
[sops-nix](https://github.com/Mic92/sops-nix):

```nix
sops.secrets."ringo/work" = { sopsFile = ./secrets/ringo.yaml; };

programs.ringo.profiles.work = {
  settings = { username = "alice"; domain = "sip.example.com"; };
  passwordFile = config.sops.secrets."ringo/work".path;
};
```

sops-nix decrypts the secret to a runtime path (e.g. under `/run/user/<uid>`);
ringo reads it fresh on each launch.

> Only the inline `password` lands at rest (in `profile.toml`, and in the store
> via the rendered TOML) — which is why it's discouraged. `passwordFile` and
> `passwordCommand` keep the secret out of both.
