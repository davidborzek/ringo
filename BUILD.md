# Building ringo

ringo statically links libre + libbaresip from vendored git submodules
(`vendor/re`, `vendor/baresip`). OpenSSL is also vendored (built from source via
`openssl-sys`). No system baresip or OpenSSL installation is needed. Audio for
ringo-flow (tone/file generation, received-audio capture for `verify-audio` and
`--save-audio`) is handled in-process by ringo's own baresip ausrc/auplay module
— no libsndfile.

## Build-time dependencies

### Required (all platforms)

| Dependency | Debian/Ubuntu | macOS (Homebrew) | Purpose |
|---|---|---|---|
| cmake | `cmake` | `cmake` | Builds libre + libbaresip |
| clang | `clang` | (Xcode) | C compiler + bindgen |
| libclang | `libclang-dev` | (llvm) | bindgen FFI bindings |
| llvm | `llvm-dev` | (llvm) | libclang headers |
| pkg-config | `pkg-config` | `pkg-config` | Library detection |
| perl | `perl` | (system) | OpenSSL vendored build |
| spandsp | `libspandsp-dev` | `spandsp` | G.722 codec |
| opus | `libopus-dev` | `opus` | Opus codec |

OpenSSL is NOT required — built from source (vendored).

### Audio backend

`ringo-phone` enables the `default-audio` feature by default. That feature lets
`build.rs` auto-detect the platform audio backend:

| Platform | Auto-detect | Build dep | Runtime dep |
|---|---|---|---|
| Linux | pulse (via pkg-config) | `libpulse-dev` | `libpulse0` |
| macOS | coreaudio (system framework) | none | none |

On Linux, pulse works on PulseAudio **and** PipeWire (via `pipewire-pulse`
compat layer). `ringo-flow` does not enable `default-audio`, so it always builds
headless and uses `aubridge` only.

To override the auto-detect, use explicit feature flags:

```bash
cargo build -p ringo-phone --features alsa  # ALSA instead of pulse
```

### Quick install

**Debian/Ubuntu (with pulse audio):**
```bash
sudo apt-get install -y cmake clang libclang-dev llvm-dev pkg-config perl \
  libspandsp-dev libopus-dev libpulse-dev
```

**Debian/Ubuntu (headless, no audio):**
```bash
sudo apt-get install -y cmake clang libclang-dev llvm-dev pkg-config perl \
  libspandsp-dev libopus-dev
```

**macOS:**
```bash
brew install cmake pkg-config spandsp opus
```

## Runtime dependencies

| Dependency | Debian/Ubuntu | macOS | Required? |
|---|---|---|---|
| ca-certificates | `ca-certificates` | (system) | Yes — TLS verification |
| spandsp | `libspandsp2` | `spandsp` | Yes — G.722 codec |
| opus | `libopus0` | `opus` | Yes — Opus codec |
| pulseaudio | `libpulse0` | — | Yes if built with pulse (default on Linux) |

## Build

### ringo-phone (default — pulse on Linux, coreaudio on macOS)
```bash
cargo build --release -p ringo-phone
```

### ringo-phone with ALSA (from source, no release binary)
```bash
cargo build --release -p ringo-phone --features alsa
```

### ringo-flow (headless, no audio)
```bash
cargo build --release -p ringo-flow
```

## Docker

```bash
docker build -f crates/ringo-flow/Dockerfile -t ringo-flow .
```

Build stage installs all required dev libs + builds OpenSSL from source.
Runtime image is `debian:bookworm-slim` with
`ca-certificates libspandsp2 libopus0` (no libssl — vendored).

## Cargo features

| Feature | Effect |
|---|---|
| `default-audio` | Auto-detect platform audio (`pulse` on Linux, `coreaudio` on macOS) |
| `pulse` | Statically link PulseAudio audio module (Linux) |
| `alsa` | Statically link ALSA audio module (Linux) |
| `coreaudio` | Statically link CoreAudio audio module (macOS) |

When no audio feature is set, `ringo-core` builds headless and defaults to
`aubridge`.

## Cross-compilation (aarch64 Linux)

```bash
sudo dpkg --add-architecture arm64
sudo apt-get update
sudo apt-get install -y gcc-aarch64-linux-gnu \
  libspandsp-dev:arm64 libopus-dev:arm64 \
  libpulse-dev:arm64

export CC=aarch64-linux-gnu-gcc
export CXX=aarch64-linux-gnu-g++
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig

cargo build --release --target aarch64-unknown-linux-gnu -p ringo-phone
```

## Troubleshooting

### `error: failed to find libclang`
Set `LIBCLANG_PATH` to the directory containing `libclang.so`:
```bash
export LIBCLANG_PATH=/usr/lib/llvm-14/lib  # Debian
export LIBCLANG_PATH=/opt/homebrew/opt/llvm/lib  # macOS
```

### `module g722.so: No such file or directory`
`libspandsp-dev` was not installed at build time. Install it and rebuild.

### Binary won't start: `libpulse.so.0: cannot open shared object file`
Install `libpulse0` (or build with `--features alsa` to use ALSA instead).

### Binary won't start: `libssl.so.*: cannot open shared object file`
This should not happen — OpenSSL is statically linked (vendored).
If you see this, ensure `openssl-sys` is in `Cargo.toml` with `vendored` feature.
