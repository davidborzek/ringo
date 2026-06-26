# ringo

> A terminal SIP softphone built on [baresip](https://github.com/baresip/baresip).

[![crates.io](https://img.shields.io/crates/v/ringo-phone)](https://crates.io/crates/ringo-phone)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

ringo is a SIP softphone with a full-featured [ratatui](https://ratatui.rs) TUI,
built on baresip. It manages multiple accounts side by side — each with its own
profile, call history and configuration — while keeping baresip running headless
in the background. It does calls, contacts, blind & attended transfer, call/dial
history, MWI, theming, remote control and headless automation.

Part of the [ringo](https://github.com/davidborzek/ringo) workspace; the crate is
`ringo-phone`, the binary is `ringo`.

📖 **Full documentation: https://davidborzek.github.io/ringo/ringo-phone/introduction.html**
— installation, profiles, the TUI and keybindings, remote control, configuration
and integrations.

## Requirements

- [baresip](https://github.com/baresip/baresip) >= 3.14 in `$PATH`
- Rust 1.85+ (to build)

## Install

```sh
brew install davidborzek/tap/ringo                          # Homebrew (macOS/Linux)
cargo install ringo-phone                                   # from crates.io
cargo install --git https://github.com/davidborzek/ringo ringo-phone   # from git
```

Pre-built binaries for Linux and macOS (x86\_64 + arm64) are also on the
[releases page](https://github.com/davidborzek/ringo/releases). Homebrew 6.0+ may
ask you to trust the third-party tap first (`brew trust --formula davidborzek/tap/ringo`) —
see [Getting started](https://davidborzek.github.io/ringo/ringo-phone/getting-started.html).

See [Getting started](https://davidborzek.github.io/ringo/ringo-phone/getting-started.html)
for installing baresip and other options.

## Getting started

```sh
ringo               # open the profile picker → Ctrl+N to create your first profile
ringo start <name>  # launch a specific profile directly
ringo list          # list all profiles
```

Fill in your SIP credentials, save, then select the profile and press Enter to
launch. Everything else — keybindings, remote control (`ringo control`), headless
sessions, `ringo.toml`, themes, contacts, hooks and integrations — is in the
[documentation](https://davidborzek.github.io/ringo/ringo-phone/introduction.html).

## License

MIT — see [LICENSE](../../LICENSE).
