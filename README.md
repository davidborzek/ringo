# ringo

> A terminal SIP softphone built on [baresip](https://github.com/baresip/baresip).

[![CI](https://github.com/davidborzek/ringo/actions/workflows/ci.yml/badge.svg)](https://github.com/davidborzek/ringo/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/ringo-phone)](https://crates.io/crates/ringo-phone)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

ringo is a SIP softphone with a full-featured ratatui TUI, built on top of baresip. It manages multiple accounts side by side — each with its own profile, call history, and configuration — while keeping baresip running headless in the background.

## Features

- **Profile picker** — fuzzy-search profile selector with inline create / edit / delete
- **Headless baresip** — spawns baresip without its built-in stdio UI, no terminal clutter
- **ratatui TUI** — status bar, command bar (`:` with tab-completion), Normal/Dial mode split, call list, DTMF, hold/resume, mute
- **Blind & attended transfer** — full call transfer support
- **Call history** — per-profile JSONL log with redial support
- **Dial history** — persistent global history with fuzzy search (Ctrl+R)
- **MWI** — message waiting indicator
- **Theming** — all UI colors configurable via `ringo.toml`, with ready-made themes
- **rofi integration** — `scripts/ringo-rofi` for WM keybinds
- **Multiple instances** — each profile gets its own dynamically assigned port

## Requirements

- [baresip](https://github.com/baresip/baresip) >= 3.14 in `$PATH`
- Rust 1.80+ (for building)
- rofi (optional, for `scripts/ringo-rofi`)

## Installation

### 1. Install baresip

ringo requires [baresip](https://github.com/baresip/baresip) >= 3.14 to be installed and available in `$PATH`. See [Supported platforms](https://github.com/baresip/baresip/wiki/Supported-platforms) for platform-specific instructions.

On most systems:

```sh
# Arch Linux
sudo pacman -S baresip

# Ubuntu / Debian (may need a PPA or manual build for >= 3.14)
sudo apt install baresip

# macOS
brew install baresip

# From source
git clone https://github.com/baresip/baresip.git
cd baresip && cmake -B build && cmake --build build && sudo cmake --install build
```

Verify the installation: `baresip -v` should show version 3.14 or later.

### 2. Install ringo

**From crates.io:**

```sh
cargo install ringo-phone
```

**From GitHub (no clone needed):**

```sh
cargo install --git https://github.com/davidborzek/ringo
```

**Pre-built binaries** for Linux and macOS (x86\_64 + arm64) are available on the
[releases page](https://github.com/davidborzek/ringo/releases).

**From source:**

```sh
cargo install --path . --root ~/.local
```

Or build and install system-wide:

```sh
cargo build --release
sudo install -m755 target/release/ringo /usr/local/bin/ringo
```

**rofi script** (optional):

```sh
cp scripts/ringo-rofi ~/.local/bin/
```

## Quick start

```sh
ringo        # open profile picker → Ctrl+N to create your first profile
```

Fill in your SIP credentials in the form, press Enter to save, then select the profile and press Enter to launch.

## Usage

```sh
ringo              # open profile picker (default)
ringo start <name> # launch a specific profile directly
ringo list         # list all profiles
ringo list --plain # one name per line (for scripting)
```

## Keybindings

### Profile picker

| Key | Action |
|-----|--------|
| `Enter` | Start selected profile |
| `Ctrl+N` | Create new profile |
| `Ctrl+E` | Edit selected profile |
| `Ctrl+Y` | Clone selected profile |
| `Ctrl+D` | Delete selected profile (confirmation popup) |
| `↑` / `↓` | Navigate (wrap-around) |
| `Esc` | Quit |

### TUI — Normal mode (default)

| Key | Action |
|-----|--------|
| `d` | Enter Dial mode |
| `a` | Accept incoming call |
| `b` / `Del` | Hang up |
| `h` | Hold |
| `r` | Resume (when on hold) |
| `m` | Toggle mute |
| `t` | Blind transfer |
| `T` | Attended transfer |
| `0-9` `*` `#` | DTMF tones (during active call) |
| `Tab` | Switch active call (when multiple calls) |
| `e` | Open event log |
| `l` | Open baresip log |
| `c` | Open call history |
| `Ctrl+R` | Fuzzy search dial history |
| `Ctrl+E` | Edit profile (no active call) |
| `Ctrl+P` | Switch profile (returns to picker) |
| `:` | Open command bar |
| `q` | Quit (confirmation prompt) |
| `Ctrl+C` | Quit immediately |

### TUI — Dial mode

| Key | Action |
|-----|--------|
| `Enter` | Dial and return to Normal mode |
| `Esc` | Cancel and return to Normal mode |
| `Backspace` | Delete character / exit to Normal mode (when empty) |
| `←` / `→` | Move cursor |
| `Home` / `End` | Jump to start / end |
| `↑` / `↓` | Navigate dial history |
| `Ctrl+R` | Fuzzy search dial history |

### Command bar

| Key | Action |
|-----|--------|
| `:` | Open command bar (from Normal mode) |
| `Tab` | Cycle tab-completion |
| `Enter` | Execute command |
| `Esc` | Close |
| `Backspace` | Delete character / close (when empty) |

Available commands: `dial <n>`, `hangup`, `accept`, `hold`, `resume`, `mute`, `transfer <uri>`, `events`, `log`, `history`, `edit`, `switch`, `help`, `quit`

### Call history view

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate entries |
| `g` / `G` | Jump to top / bottom |
| `Enter` | Copy peer to dial input (redial) |
| `/` | Search |
| `d` | Delete selected entry |
| `D` | Clear entire history |
| `c` / `Esc` | Close |

### Event log / baresip log view

| Key | Action |
|-----|--------|
| `↑` / `↓` | Scroll |
| `g` / `G` | Jump to top / bottom |
| `e` | Toggle / switch to event log |
| `l` | Toggle / switch to baresip log |
| `c` | Switch to call history |
| `Esc` | Close |

## Configuration

Global config lives at `~/.config/ringo/ringo.toml`.

### Picker subtitle

```toml
[picker]
# Fields shown next to each profile name in the picker.
# Available: aor, username, domain, display_name, transport,
#            auth_user, outbound, stun_server, media_enc
info = ["aor"]   # default
```

### Theme

All UI colors are configurable. Colors accept named values or `#rrggbb` hex.

| Role | Default | Used for |
|------|---------|----------|
| `accent` | `"cyan"` | Logo, picker selection, DTMF input, history popup |
| `subtle` | `"dark_gray"` | Hints, log text, subtitles, unfocused labels |
| `success` | `"green"` | Registered status, established call, toggle on |
| `danger` | `"red"` | Muted indicator, missed calls, registration failed |
| `attention` | `"yellow"` | Selected call, ringing, MWI, focused form field |
| `transfer` | `"magenta"` | Transfer mode input |

```toml
[theme]
accent    = "cyan"
subtle    = "dark_gray"
success   = "green"
danger    = "red"
attention = "yellow"
transfer  = "magenta"
```

Supported names: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `gray`,
`dark_gray`, `light_red`, `light_green`, `light_yellow`, `light_blue`, `light_magenta`,
`light_cyan`, `white`, and hex values like `"#ff8800"`.

Ready-made themes in [`themes/`](themes/):
[Catppuccin Mocha](themes/catppuccin-mocha.toml) · [Gruvbox](themes/gruvbox.toml) · [Nord](themes/nord.toml) · [Tokyo Night](themes/tokyo-night.toml)

### baresip

ringo auto-detects your system's baresip module path and audio driver. All values can be overridden in `ringo.toml`:

```toml
[baresip]
# Path to baresip modules — auto-detected via pkg-config or known system paths.
# See: https://github.com/baresip/baresip/wiki/Modules
module_path = "/usr/lib/baresip/modules"

# Audio backend module loaded by baresip.
# Common values: "alsa", "pulse", "pipewire", "coreaudio"
# See: https://github.com/baresip/baresip/wiki/Configuration#audio
audio_driver = "pipewire"

# Audio device names passed to the driver (e.g. "default", "hw:0,0", a PipeWire/PulseAudio sink name).
# Each can be set independently; all default to "default".
audio_player_device = "default"
audio_source_device = "default"
audio_alert_device  = "default"

# CA certificate file for SIP TLS — auto-detected from common system paths.
sip_cafile = "/etc/ssl/certs/ca-certificates.crt"

# CA certificate directory for SIP TLS — auto-detected on Linux, disabled on macOS.
# Set to "" to explicitly disable.
sip_capath = "/etc/ssl/certs"

# Arbitrary baresip config overrides — appended at the end of the
# generated config. Last value wins, so these override defaults.
# ⚠️  Incorrect values can break ringo.
# See: https://github.com/baresip/baresip/wiki/Configuration
[baresip.extra]
dns_server = "10.0.0.1:53"
call_max_calls = "8"
```

All keys are optional; omitting a key falls back to auto-detection.

### Hooks

Run shell commands when certain events occur. Each hook receives environment variables with context about the event.

```toml
[[hooks]]
event = "profile_loaded"
command = "bash ~/.config/ringo/hooks/on_profile_loaded.sh"

[[hooks]]
event = "profile_loaded"
command = "notify-send 'ringo' \"Profile $RINGO_PROFILE started\""
```

| Event | Trigger |
|-------|---------|
| `profile_loaded` | After a profile is loaded and baresip is spawned |

**Environment variables** passed to each hook:

| Variable | Description |
|----------|-------------|
| `RINGO_EVENT` | Event name (e.g. `profile_loaded`) |
| `RINGO_PROFILE` | Profile name |
| `RINGO_PROFILE_JSON` | Profile data as JSON (excludes `password`) |

Hooks run in background threads and do not block the UI. Errors are logged to `/tmp/ringo-hooks.log`.

## Profile config

Profiles are stored as TOML at `~/.config/ringo/profiles/<name>/profile.toml`:

```toml
username     = "user123"
password     = "secret"
domain       = "sip.example.com"
display_name = "My Name"              # optional
transport    = "tls"                  # optional: udp, tcp, tls
outbound     = "sip:proxy.example.com" # optional
stun_server  = "stun:stun.example.com" # optional
media_enc    = "dtls_srtp"            # optional
notify       = true                   # desktop notifications (default: true)
mwi          = true                   # message waiting indicator (default: true)
```

## File locations

| Path | Description |
|------|-------------|
| `~/.config/ringo/ringo.toml` | Global config |
| `~/.config/ringo/profiles/<name>/profile.toml` | Profile config |
| `~/.config/ringo/profiles/<name>/call_history` | Per-profile call history (JSONL) |
| `~/.local/share/ringo/history` | Global dial history |
| `/tmp/ringo-<name>-<ts>/` | Runtime temp dir (auto-cleaned) |
| `/tmp/ringo-hooks.log` | Hook execution log |

## Shell completions

ringo supports dynamic shell completions — profile names are completed from your actual profiles at `~/.config/ringo/profiles/`.

**fish** — add to `~/.config/fish/config.fish`:

```fish
COMPLETE=fish ringo | source
```

**bash** — add to `~/.bashrc`:

```bash
source <(COMPLETE=bash ringo)
```

**zsh** — add to `~/.zshrc`:

```zsh
source <(COMPLETE=zsh ringo)
```

After sourcing, `ringo start <Tab>` will complete profile names.

## rofi integration

```sh
cp scripts/ringo-rofi ~/.local/bin/

# sway / i3
bindsym $mod+p exec ringo-rofi
```

`ringo-rofi` uses `$TERMINAL` if set, otherwise tries `kitty`, `alacritty`, `foot`, `wezterm`, `xterm`.

## tmux integration

```sh
cp scripts/ringo-tmux ~/.local/bin/

# then just run:
ringo-tmux
```

`ringo-tmux` uses `fzf` for multi-select profile picking and opens each
selected profile in its own tmux pane within a session named `ringo`.
Requires: `tmux`, `fzf`.

## Call history format

One JSON object per line:

```json
{"ts":"2024-01-15 14:30:05","dir":"outgoing","peer":"sip:alice@example.com","duration":"02:05:13","duration_secs":7513}
```

```sh
cat ~/.config/ringo/profiles/<name>/call_history | jq .
```

## Contributing

Contributions are welcome. Please open an issue before submitting large changes so we can discuss the approach first.

```sh
cargo build       # build
cargo test        # run tests
cargo clippy      # lint
```

## License

MIT — see [LICENSE](LICENSE).
