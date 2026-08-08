# Configuration

Global config lives at `~/.config/ringo/ringo.toml`. Everything below is optional.

## Splitting the config across files

`include` folds other TOML files into `ringo.toml`:

```toml
include = ["theme.toml", "~/.config/ringo/work.toml"]
```

- Relative paths resolve against the directory of the file naming them, so
  `theme.toml` means `~/.config/ringo/theme.toml`. A leading `~/` expands to
  your home directory.
- Includes are applied in the order listed, then the including file's own keys
  go on top. So a later include beats an earlier one, and whatever you write in
  `ringo.toml` beats all of them.
- Tables merge per key. An included `[theme]` that sets only `accent` leaves
  your local `subtle` alone. Arrays — `hooks`, `picker.info` — are replaced
  whole, not concatenated.
- Included files may include further files.
- A file that is missing or unparseable is logged and skipped; the rest of your
  config still loads.

This is what makes generated themes practical. [matugen](https://github.com/InioX/matugen),
for instance, can render a ringo theme from your wallpaper — point a template at
`~/.config/ringo/theme.toml`:

```toml
# matugen config
[templates.ringo]
input_path = "~/.config/matugen/templates/ringo.toml"
output_path = "~/.config/ringo/theme.toml"
```

```toml
# ~/.config/matugen/templates/ringo.toml
[theme]
accent    = "{{colors.primary.default.hex}}"
subtle    = "{{colors.outline.default.hex}}"
success   = "{{colors.tertiary.default.hex}}"
danger    = "{{colors.error.default.hex}}"
attention = "{{colors.secondary.default.hex}}"
transfer  = "{{colors.primary_fixed_dim.default.hex}}"
```

With `include = ["theme.toml"]` in `ringo.toml`, re-running matugen re-themes the
phone on the next start, and nothing hand-written gets overwritten.

## Picker subtitle

```toml
[picker]
# Fields shown next to each profile name in the picker. Available: aor, username,
# domain, display_name, transport, auth_user, outbound, stun_server, media_enc.
info = ["aor"]   # default
```

## Theme

All UI colors are configurable — named values or `#rrggbb` hex.

| Role | Default | Used for |
|------|---------|----------|
| `accent` | `cyan` | Logo, picker selection, DTMF input, history popup |
| `subtle` | `dark_gray` | Hints, log text, subtitles, unfocused labels |
| `success` | `green` | Registered, established call, toggle on |
| `danger` | `red` | Muted, missed calls, registration failed |
| `attention` | `yellow` | Selected call, ringing, MWI, focused field |
| `transfer` | `magenta` | Transfer-mode input |

```toml
[theme]
accent    = "cyan"
subtle    = "dark_gray"
success   = "green"
danger    = "red"
attention = "yellow"
transfer  = "magenta"
```

Ready-made themes (Catppuccin Mocha, Gruvbox, Nord, Tokyo Night) live in
[`themes/`](https://github.com/davidborzek/ringo/tree/main/themes). Drop one next
to your config and name it in `include` instead of copying the block:

```toml
include = ["tokyo-night.toml"]
```

## baresip

ringo auto-detects the audio driver; override any of these
in `ringo.toml`:

```toml
[baresip]
audio_driver = "pipewire"                   # an audio driver compiled into your build; auto-detected if unset
audio_player_device = "default"
audio_source_device = "default"
audio_alert_device  = "default"
sip_cafile   = "/etc/ssl/certs/ca-certificates.crt"  # SIP TLS CA file
sip_capath   = "/etc/ssl/certs"                       # CA dir ("" to disable)

# Arbitrary baresip config overrides, appended last (last value wins).
# ⚠️ Incorrect values can break ringo. See the baresip Configuration wiki.
[baresip.extra]
dns_server     = "10.0.0.1:53"
call_max_calls = "8"
```

## Contacts

Contacts live at `~/.config/ringo/contacts.toml`; names resolve in the call list
and history, and numbers match across formats (`01555…`, `+491555…`, `491555…`).

```toml
[[contacts]]
name = "Alice"
numbers = ["+491555123456", "alice.work"]
```

Manage them in the TUI (contacts overlay → `a`/`e`/`d`) or with `$EDITOR` (`E`).

## Hooks

Run shell commands on events; each hook gets context via environment variables and
runs in a background thread (errors go to the log at
`$XDG_STATE_HOME/ringo/<name>.log`, default `~/.local/state/ringo/<name>.log`).

```toml
[[hooks]]
event = "call_incoming"
command = "notify-send 'ringo' \"Call from $(echo $RINGO_EVENT_DATA | jq -r .number)\""
```

| Event | Trigger | Event data |
|-------|---------|------------|
| `profile_loaded` | Profile loaded, baresip spawned | — |
| `call_incoming` | Incoming call | `call_id`, `number`, `display_name` |
| `call_outgoing` | Outgoing call initiated | `call_id`, `number` |
| `call_ended` | Call closed | `call_id`, `number`, `direction`, `duration_secs`, `reason`, `error` |

Each hook receives `RINGO_EVENT`, `RINGO_PROFILE`, `RINGO_PROFILE_JSON` (no
password) and `RINGO_EVENT_DATA` (JSON).
