# ringo

> A terminal SIP softphone built on [baresip](https://github.com/baresip/baresip).

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)

ringo is a SIP softphone with a full-featured ratatui TUI, built on top of baresip. It manages multiple accounts side by side — each with its own profile, call history, and configuration — while keeping baresip running headless in the background.

## Features

- **Profile picker** — fuzzy-search profile selector with inline create / edit / delete
- **Headless baresip** — spawns baresip without its built-in stdio UI, no terminal clutter
- **ratatui TUI** — call list, dial input with cursor editing, DTMF, hold/resume, mute
- **Blind & attended transfer** — full call transfer support
- **Call history** — per-profile JSONL log with redial support
- **Dial history** — persistent global history with fuzzy search (Ctrl+R)
- **MWI** — message waiting indicator
- **Theming** — all UI colors configurable via `ringo.toml`, with ready-made themes
- **rofi integration** — `scripts/ringo-rofi` for WM keybinds
- **Multiple instances** — each profile gets its own dynamically assigned port

## Requirements

- [baresip](https://github.com/baresip/baresip) in `$PATH`
- Rust 1.80+ (for building)
- rofi (optional, for `scripts/ringo-rofi`)

## Installation

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
ringo sync         # regenerate all profile configs from shared template
```

## Keybindings

### Profile picker

| Key | Action |
|-----|--------|
| `Enter` | Start selected profile |
| `Ctrl+N` | Create new profile |
| `Ctrl+E` | Edit selected profile |
| `Ctrl+D` | Delete selected profile (confirmation popup) |
| `Esc` | Quit |

### TUI — always available

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | Quit (hangs up all active calls) |
| `Ctrl+P` | Switch profile (returns to picker) |
| `Enter` | Dial |
| `Esc` | Clear dial input / cancel history navigation |
| `Backspace` / `Delete` | Edit dial input |
| `←` / `→` | Move cursor in dial input |
| `Home` / `End` | Jump to start / end of dial input |
| `↑` / `↓` | Navigate dial history (when no log open) |
| `Ctrl+R` | Fuzzy search dial history |
| `Tab` | Switch active call (when multiple calls) |
| `e` | Toggle event log |
| `l` | Toggle baresip process log |
| `c` | Toggle call history (when no active calls) |

### TUI — during a call

| Key | Action |
|-----|--------|
| `a` | Accept incoming call |
| `b` / `Del` | Hang up |
| `h` | Hold |
| `r` | Resume (when on hold) |
| `m` | Toggle mute |
| `t` | Blind transfer |
| `T` | Attended transfer |
| `0-9` `*` `#` | DTMF tones |

### Call history view

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate entries |
| `Enter` | Copy peer to dial input (redial) |
| `d` | Delete selected entry |
| `D` | Clear entire history |
| `c` / `Esc` | Close |

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
| `~/.config/ringo/config` | Shared baresip config template |
| `~/.local/share/ringo/history` | Global dial history |
| `/tmp/ringo-<name>-<ts>/` | Runtime temp dir (auto-cleaned) |

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
