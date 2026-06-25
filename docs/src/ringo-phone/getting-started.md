# Getting started

## Install

### 1. baresip

ringo needs [baresip](https://github.com/baresip/baresip) ≥ 3.14 in your `$PATH`
(`baresip -v` to check). See
[Supported platforms](https://github.com/baresip/baresip/wiki/Supported-platforms);
on most systems:

```sh
sudo pacman -S baresip   # Arch
sudo apt install baresip # Debian/Ubuntu (may need >= 3.14 from a PPA/source)
brew install baresip     # macOS
```

### 2. ringo

**Pre-built binaries** for Linux and macOS (x86\_64 + arm64) are on the
[releases page](https://github.com/davidborzek/ringo/releases) — download, extract
and put `ringo` on your `$PATH`.

**From crates.io:**

```sh
cargo install ringo-phone
```

**From GitHub (no clone needed):**

```sh
cargo install --git https://github.com/davidborzek/ringo ringo-phone
```

## Quick start

```sh
ringo        # open the profile picker → Ctrl+N to create your first profile
```

Fill in your SIP credentials in the form, press Enter to save, then select the
profile and press Enter to launch. See [Profiles](profiles.md) for the fields.

## Usage

```sh
ringo              # open the profile picker (default)
ringo start <name> # launch a specific profile directly
ringo list         # list all profiles
ringo list --plain # one name per line (for scripting)
ringo list --json  # as a JSON array
```

From here, [Using the TUI](tui.md) covers the keybindings, and
[Remote control](remote-control.md) covers driving a running session from a script.
