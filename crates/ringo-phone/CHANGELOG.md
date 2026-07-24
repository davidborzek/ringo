# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.14.0](https://github.com/davidborzek/ringo/compare/ringo-phone-v0.13.0...ringo-phone-v0.14.0) - 2026-07-24

### Bug Fixes

- hold the active call when answering a second incoming call
- act on the selected call across multi-call operations

### Features

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
