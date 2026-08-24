# Getting started

## Install

baresip is built in and statically linked — no separate `baresip` install needed.

**Homebrew (macOS / Linux):**

```sh
brew install davidborzek/tap/ringo
```

**Arch Linux (AUR)** — prebuilt, via any AUR helper:

```sh
yay -S ringo-phone-bin   # or: paru -S ringo-phone-bin
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

**Nix (flake):**

```sh
nix profile install github:davidborzek/ringo#ringo-phone
```

Or manage ringo and its SIP profiles declaratively with the
[Home-Manager module](home-manager.md).

Building ringo from source means compiling OpenSSL, libre and libbaresip before
the Rust code — ten minutes or so, more on a small machine. A binary cache holds
the result for every platform the flake supports, so enable it first and the
install is a download instead:

```sh
cachix use ringo
```

On NixOS, in your configuration:

```nix
nix.settings = {
  substituters = [ "https://ringo.cachix.org" ];
  trusted-public-keys = [ "ringo.cachix.org-1:Zfdjuf1dHz+C0a4BOKfPbBOxTcHmKy3c2Ddr0tUiawk=" ];
};
```

The cache is filled from the default branch, so `nix run
github:davidborzek/ringo` hits it. Pointing the overlay at your own nixpkgs
produces a different derivation and builds from source again.

To pin a released version, name its tag — a flake has no version of its own, the
git ref is the version:

```sh
nix profile install github:davidborzek/ringo/ringo-phone-v0.14.0#ringo-phone
```

Releases are tagged per crate, so `ringo-phone-v0.14.0` is the tag for the phone.
The flake at that tag still provides `ringo-flow` too, in whatever version that
commit carried.

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
