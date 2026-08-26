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
- Included files may include further files, up to eight levels deep. A file that
  ends up including itself, directly or through a chain, is reported and skipped
  rather than followed — but two files pulling in the same third file is fine and
  applies it both times.
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
# domain, display_name, transport, auth_user, outbound, stun_server, media_enc,
# notes, and metadata.<key> (see below).
info = ["aor"]   # default

# How much of the RINGO wordmark to show above the search box:
#   auto  (default) block letters when the terminal has room, one line if not
#   full            always the block letters
#   small           always the one-line wordmark
#   off             none at all
logo = "auto"

# Order of the profile list:
#   recent  (default) most recently started first, then the rest by name
#   name              always alphabetical
order = "recent"
```

The recent order is remembered in `~/.local/state/ringo/recent` — one name per
line, most recent first, written when a profile actually starts. It lives in the
state directory rather than the config because it is something ringo observes,
not something you set; deleting the file just resets the order.

A profile can carry free-form key/value pairs that ringo never interprets, and
the picker can show them:

```toml
# ~/.config/ringo/profiles/<name>/profile.toml
[metadata]
env = "staging"
```

```toml
# ~/.config/ringo/ringo.toml
[picker]
info = ["aor", "metadata.env"]
```

This is for whatever tells your profiles apart that the SIP fields do not — the
environment an account belongs to, the tenant it serves, the extension it
answers on. A key that a profile does not set is simply left out, so profiles
can carry different ones.

## Theme

All UI colors are configurable — named values or `#rrggbb` hex.

| Role        | Default     | Used for                                          |
| ----------- | ----------- | ------------------------------------------------- |
| `accent`    | `cyan`      | Logo, picker selection, DTMF input, history popup |
| `subtle`    | `dark_gray` | Hints, log text, subtitles, unfocused labels      |
| `success`   | `green`     | Registered, established call, toggle on           |
| `danger`    | `red`       | Muted, missed calls, registration failed          |
| `attention` | `yellow`    | Selected call, ringing, MWI, focused field        |
| `transfer`  | `magenta`   | Transfer-mode input                               |

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

## Sounds

ringo plays five alert tones, each with a default built into the binary:

| Alert      | When                                                  | Plays     | Default                 |
| ---------- | ----------------------------------------------------- | --------- | ----------------------- |
| `ring`     | incoming call                                         | on repeat | rising G–C–E chime, every 2.5 s |
| `ringback` | outgoing call, the other end is ringing               | on repeat | 425 Hz, 1 s / 4 s       |
| `busy`     | your outgoing call was rejected as busy (486/600/603) | once      | 425 Hz, 480 ms / 480 ms |
| `error`    | your outgoing call failed for another reason          | once      | 425 Hz, 240 ms / 240 ms |
| `message`  | a new voicemail arrived while ringo was running (MWI) | once      | two descending notes    |

Every default is generated at 48 kHz — ringo embeds no audio files at all.

The three call-progress tones come from their specification, so the cadence is
exact and the loop has no seam. The values are the German ones (`[de]` in
Asterisk's `indications.conf`): `busy` is the ordinary engaged tone, `error` the
faster congestion tone that says the network could not put the call through.

To silence a ringtone that is sounding right now without configuring anything,
press `s` (or `:silence`, or `ringo ctl silence`). It stops your side only: the
caller keeps hearing ringback, the call stays there to answer, and the next call
rings again.

`ring` and `message` are chimes — struck notes with a decay, the way a modern
ringtone is a short motif in a cadence rather than a continuous tone. Nothing
specifies what an incoming call should sound like, so these are a choice rather
than a standard; replace them like anything else below.

There are three ways to replace any of them.

**Drop in a file.** Put `ring.wav`, `ringback.wav`, `busy.wav`, `error.wav` or
`message.wav` into `~/.config/ringo/sounds/` and ringo uses it. No config needed.

**Write a tone.** A value starting with `tone:` is generated instead of read
from disk, in the notation Asterisk's `indications.conf` uses — so any of its 50
country zones can be pasted in. Elements are separated by commas, each is
`freq[+freq…]/milliseconds`, and a frequency of `0` is silence:

```toml
[sounds]
ringback = "tone:440+480/2000,0/4000"    # North America
busy     = "tone:480+620/500,0/500"      # North America
error    = "tone:914/330,1371/330,1777/330,0/1000"   # the three-tone SIT
```

The British double ring shows why an element list beats a simple on/off pair:
`ringback = "tone:400+450/400,0/200,400+450/400,0/2000"`.

Asterisk's `*` (modulation) and `!` (play once) are not supported; ringo says so
by name rather than mis-playing them. A spec that cannot be read is logged and
leaves the built-in tone in place.

**Or name a file explicitly:**

```toml
[sounds]
ring     = "~/media/nokia.wav"   # absolute path, or ~/ for your home directory
ringback = "old-ringback.wav"    # bare name: relative to ~/.config/ringo/sounds/
busy     = "off"                 # "off" (or "none") silences this alert
```

An explicit entry beats a drop-in file of the same name, and any alert you leave
out keeps its built-in tone.

Your file is played by baresip straight from disk, so only its path is held in
memory and there is no size limit. The flip side is that it is read when the
alert fires: a few hundred kB from the page cache is nothing, but a very large
file on cold or networked storage delays the call it is announcing. Keep a
ringtone to a few seconds and it never matters.

The path must not contain a comma — baresip's player reads one as a
repeat/delay suffix and truncates the name there. ringo says so at start rather
than letting it fail at the first call.

What baresip's loader accepts is:

- WAV (RIFF), with the `fmt ` chunk first — a leading `LIST`/metadata chunk makes
  the file unreadable for it.
- 16-bit PCM, A-law or mu-law. Not 8-bit or 24-bit PCM, not float, and not
  `WAVE_FORMAT_EXTENSIBLE`, which many converters emit by default.
- Any sample rate and channel count your audio driver will open. Rates other
  than 8/16/32/44.1/48 kHz work with PulseAudio and PipeWire; ALSA on a raw
  `hw:` device can refuse them.

ringo checks all of this while it starts, so a file it cannot use is a line in
the log rather than silence at the first call:

```
WARN: sounds: ring: '/home/you/tune.wav' is WAVE_FORMAT_EXTENSIBLE, which
      baresip's loader rejects — re-encode it as plain 16-bit PCM
```

Whatever fails — bad path, wrong format, a rate the driver turns down — falls
back to the built-in tone. A typo never leaves an incoming call silent.

`ffmpeg` produces a file that always works:

```sh
ffmpeg -i nokia.mp3 -ac 1 -ar 48000 -c:a pcm_s16le ~/.config/ringo/sounds/ring.wav
```

The config is read once at start, so a _different_ file takes effect on the next
start (or after a profile switch) — but overwriting the file you already
configured takes effect immediately, since it is opened fresh on every alert.

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

| Event            | Trigger                         | Event data                                                           |
| ---------------- | ------------------------------- | -------------------------------------------------------------------- |
| `profile_loaded` | Profile loaded, baresip spawned | —                                                                    |
| `call_incoming`  | Incoming call                   | `call_id`, `number`, `display_name`                                  |
| `call_outgoing`  | Outgoing call initiated         | `call_id`, `number`                                                  |
| `call_ended`     | Call closed                     | `call_id`, `number`, `direction`, `duration_secs`, `reason`, `error` |

Each hook receives `RINGO_EVENT`, `RINGO_PROFILE`, `RINGO_PROFILE_JSON` (no
password) and `RINGO_EVENT_DATA` (JSON).
