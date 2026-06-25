<h1 class="lockup">
  <img class="lockup-mark" src="../logo.svg" alt="" />
  <span class="lockup-text"><span class="lockup-name">ringo</span><span class="lockup-sub">phone</span></span>
</h1>

**ringo** is a terminal SIP softphone built on
[baresip](https://github.com/baresip/baresip), with a full
[ratatui](https://ratatui.rs) TUI. It manages multiple accounts side by side —
each with its own profile, call history and configuration — while keeping baresip
running headless in the background.

It's part of the [ringo](https://github.com/davidborzek/ringo) workspace; the
crate is `ringo-phone`, the binary is `ringo`.

## Features

- **Profile picker** — fuzzy-search selector with inline create / edit / clone / delete.
- **Headless baresip** — spawns baresip without its stdio UI; no terminal clutter.
- **ratatui TUI** — status bar, command bar (`:` with tab-completion), Normal/Dial
  split, call list, DTMF, hold/resume, mute.
- **Contact book** — TOML contacts with fuzzy number matching and `$EDITOR` editing.
- **Blind & attended transfer** with a contact picker.
- **Call history** (per-profile, redial) and **dial history** (global, `Ctrl+R`).
- **MWI** message-waiting indicator.
- **Theming** — every UI color configurable, with ready-made themes.
- **Remote control** — drive a running session from another terminal or a script.
- **Multiple instances** — each profile gets its own dynamically assigned port.

## Next steps

- [Getting started](getting-started.md) — install and launch your first profile.
- [Profiles](profiles.md) and [Configuration](configuration.md) — set up accounts
  and tune the UI / baresip.
- [Using the TUI](tui.md) — modes and keybindings.
- [Remote control](remote-control.md) — drive sessions from scripts.

> For scripted, multi-agent telephony **testing** (assertions, audio, HTTP), see
> the companion tool [ringo-flow](../ringo-flow/introduction.md).
