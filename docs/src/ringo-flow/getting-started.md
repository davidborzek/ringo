# Getting started

## Install

### 1. baresip

ringo-flow needs [baresip](https://github.com/baresip/baresip) ≥ 3.14 in your
`$PATH` (`baresip -v` to check). See
[Supported platforms](https://github.com/baresip/baresip/wiki/Supported-platforms)
for install instructions (`pacman -S baresip`, `brew install baresip`, …).

### 2. ringo-flow

**Homebrew (macOS / Linux)** — also installs baresip, so you can skip step 1:

```sh
brew install davidborzek/tap/ringo-flow
```

**Pre-built binaries** for Linux and macOS (x86\_64 + arm64) are on the
[releases page](https://github.com/davidborzek/ringo/releases) — download, extract
and put `ringo-flow` on your `$PATH`.

**From crates.io:**

```sh
cargo install ringo-flow
```

**From GitHub (no clone needed):**

```sh
cargo install --git https://github.com/davidborzek/ringo ringo-flow
```

**From a workspace checkout** (no install):

```sh
cargo run -p ringo-flow -- run scenario.rhai
```

> Homebrew 6.0+ requires third-party taps to be trusted before use. If `brew
> install` prompts you to trust the tap, accept it — or trust it up front:
>
> ```sh
> brew tap davidborzek/tap
> brew trust --formula davidborzek/tap/ringo-flow
> ```

## Run a scenario

Credentials and the SIP domain come from the environment (via
[`env(...)`](api/environment.md#env)), so nothing sensitive lives in the script:

```sh
SIP_DOMAIN=example.com A_USER=alice A_PASS=… B_USER=bob B_PASS=… \
  ringo-flow run scenario.rhai
```

```sh
ringo-flow run scenario.rhai     # one file
ringo-flow run scenarios/        # a directory (all *.rhai, recursively)
ringo-flow check scenario.rhai   # syntax-check only (no baresip)
```

The exit code is non-zero if any scenario fails.

## Useful flags

- `--scenario <pattern>` — run a subset by name (`re:` for a regex).
- `--tag <tag>` / `--exclude-tag <tag>` — filter by tag (repeatable, comma-separated).
- `--env-file FILE` — load variables for `env(...)` (a sibling `<scenario>.env`
  is layered on top per file).
- `--log [<file>]` — write the backend/SIP log to stderr (or a file); off by default.
- `--sip-trace` — trace every SIP request/response (to the log; stderr if no `--log`).
- `--save-audio` — save sent/received WAVs to the working directory.
- `--json` — emit NDJSON events (for CI).
- `-q` / `-v`, `--no-color`.

See `ringo-flow run --help` for the full list.
