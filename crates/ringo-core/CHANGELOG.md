# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.13.0](https://github.com/davidborzek/ringo/compare/ringo-core-v0.12.0...ringo-core-v0.13.0) - 2026-07-15

### Features

- *(ringo-core)* call deflection via SIP 302 ([#61](https://github.com/davidborzek/ringo/pull/61))
- *(ringo-core)* live call quality and codec selection

## [0.12.0](https://github.com/davidborzek/ringo/compare/ringo-core-v0.11.0...ringo-core-v0.12.0) - 2026-06-30

### Features

- live audio streaming in and out of a call ([#51](https://github.com/davidborzek/ringo/pull/51))
- *(ringo-flow)* run every agent in its own process ([#48](https://github.com/davidborzek/ringo/pull/48))
- *(ringo-flow)* assert on received DTMF ([#45](https://github.com/davidborzek/ringo/pull/45))
- *(ringo-flow)* RTP media stats + MOS assertions ([#43](https://github.com/davidborzek/ringo/pull/43))

## [0.11.0](https://github.com/davidborzek/ringo/compare/ringo-core-v0.10.1...ringo-core-v0.11.0) - 2026-06-27

### Bug Fixes

- *(ringo-core)* quote outbound URI in generated account line ([#30](https://github.com/davidborzek/ringo/pull/30))

### Documentation

- GitHub Pages site (ringo-phone + ringo-flow) ([#31](https://github.com/davidborzek/ringo/pull/31))

### Features

- call deflection via SIP 302 ([#34](https://github.com/davidborzek/ringo/pull/34))
- replace process-based baresip backend with FFI backend ([#33](https://github.com/davidborzek/ringo/pull/33))

## [0.10.0](https://github.com/davidborzek/ringo/compare/ringo-core-v0.9.0...ringo-core-v0.10.0) - 2026-06-22

### Features

- *(ringo-flow)* telephony scenario test runner on baresip ([#18](https://github.com/davidborzek/ringo/pull/18))
