# Configuration

Global config lives at `~/.config/ringo/ringo.toml`. Everything below is optional.

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
[`themes/`](https://github.com/davidborzek/ringo/tree/main/themes).

## baresip

ringo auto-detects the baresip module path and audio driver; override any of these
in `ringo.toml`:

```toml
[baresip]
module_path  = "/usr/lib/baresip/modules"  # baresip modules
audio_driver = "pipewire"                   # alsa | pulse | pipewire | coreaudio
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
