# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.16.0](https://github.com/davidborzek/ringo/compare/ringo-phone-v0.15.0...ringo-phone-v0.16.0) - 2026-08-31

### Features

- *(ringo-phone)* make the picker legible, and stop it crashing ([#109](https://github.com/davidborzek/ringo/pull/109))
- *(ringo-phone)* show profile metadata in the picker subtitle ([#107](https://github.com/davidborzek/ringo/pull/107))
- *(ringo-phone)* register and unregister as commands ([#106](https://github.com/davidborzek/ringo/pull/106))
- *(ringo-phone)* deafen, and a hint bar that fits ([#104](https://github.com/davidborzek/ringo/pull/104))

## [0.15.0](https://github.com/davidborzek/ringo/compare/ringo-phone-v0.14.0...ringo-phone-v0.15.0) - 2026-08-24

### Bug Fixes

- *(deps)* update rust crate toml to v1 ([#91](https://github.com/davidborzek/ringo/pull/91))
- *(ringo-phone)* clear the screen without asking where the cursor is ([#92](https://github.com/davidborzek/ringo/pull/92))
- *(deps)* update rust dependencies (non-major) ([#81](https://github.com/davidborzek/ringo/pull/81))

### Features

- customizable alert sounds ([#101](https://github.com/davidborzek/ringo/pull/101))
- *(ringo-phone)* include other TOML files from ringo.toml ([#93](https://github.com/davidborzek/ringo/pull/93))

## [0.14.0](https://github.com/davidborzek/ringo/compare/ringo-phone-v0.13.0...ringo-phone-v0.14.0) - 2026-07-29

### Bug Fixes

- hold the active call when answering a second incoming call
- act on the selected call across multi-call operations

### Features

- resolve the SIP password from a file or command ([#70](https://github.com/davidborzek/ringo/pull/70))
- add `info` command to open the call-details overlay
- show sent custom headers in outgoing call details
- show configurable inbound-header views for incoming calls
- accept tel:/callto: dial targets and show attended-transfer state
- add auto-hold profile setting and responsive profile form

## [0.13.0](https://github.com/davidborzek/ringo/compare/ringo-phone-v0.12.0...ringo-phone-v0.13.0) - 2026-07-23

### Bug Fixes

- auto-resume the new current call with more than two calls
- auto-hold the active call when placing a second one
- auto-resume the held call after an attended transfer ends
- signal SIP hold when starting an attended transfer
- *(ringo-phone)* open $EDITOR on the real terminal from a running session
- *(ringo-phone)* exit cleanly when leaving the picker without a selection
- wait for de-REGISTER before teardown, switch profiles instantly

### Features

- accept human-formatted phone numbers when dialing

## [0.12.0](https://github.com/davidborzek/ringo/compare/ringo-phone-v0.11.1...ringo-phone-v0.12.0) - 2026-07-17

### Bug Fixes

- *(ringo-phone)* keep baresip's raw stdout off the TUI screen
- *(ringo-phone)* wrap keybind hints and keep selection in view
- *(ringo-phone)* smoother TUI rendering, cleaner log view

### Features

- *(ringo-phone)* call deflection via SIP 302 ([#61](https://github.com/davidborzek/ringo/pull/61))
- *(ringo-phone)* live call quality and codec selection
- *(ringo-phone)* tabbed profile form with descriptions

### Refactor

- *(ringo-phone)* TUI polish — log pager, unified dialogs, which-key hints
- *(ringo-phone)* move secondary views into modal overlays

## [0.11.1](https://github.com/davidborzek/ringo/compare/ringo-phone-v0.11.0...ringo-phone-v0.11.1) - 2026-06-30

### Features

- *(ringo-phone)* enable catchall UA by default ([#49](https://github.com/davidborzek/ringo/pull/49))
- *(ringo-flow)* run every agent in its own process ([#48](https://github.com/davidborzek/ringo/pull/48))

## [0.11.0](https://github.com/davidborzek/ringo/compare/ringo-phone-v0.10.1...ringo-phone-v0.11.0) - 2026-06-27

### Documentation

- polish ringo-flow API reference, add Homebrew, llms.txt & .d.rhai ([#32](https://github.com/davidborzek/ringo/pull/32))
- GitHub Pages site (ringo-phone + ringo-flow) ([#31](https://github.com/davidborzek/ringo/pull/31))

### Features

- call deflection via SIP 302 ([#34](https://github.com/davidborzek/ringo/pull/34))
- replace process-based baresip backend with FFI backend ([#33](https://github.com/davidborzek/ringo/pull/33))

## [0.10.0](https://github.com/davidborzek/ringo/compare/v0.9.0...v0.10.0) - 2026-06-22

### Features

- *(ringo-flow)* telephony scenario test runner on baresip ([#18](https://github.com/davidborzek/ringo/pull/18))
