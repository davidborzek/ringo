# Getting started

## Install

baresip is built in and statically linked — no separate `baresip` install needed.

**Homebrew (macOS / Linux):**

```sh
brew install davidborzek/tap/ringo
```

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

> Homebrew 6.0+ requires third-party taps to be trusted before use. If `brew
> install` prompts you to trust the tap, accept it — or trust it up front:
>
> ```sh
> brew tap davidborzek/tap
> brew trust --formula davidborzek/tap/ringo
> ```

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
