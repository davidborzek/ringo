# Getting started

## Install

baresip is built in and statically linked — no separate `baresip` install needed.

**Homebrew (macOS / Linux):**

```sh
brew install davidborzek/tap/ringo-flow
```

**Arch Linux (AUR)** — prebuilt, via any AUR helper:

```sh
yay -S ringo-flow-bin   # or: paru -S ringo-flow-bin
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
cargo run -p ringo-flow -- run scenario.js
```

**Nix (flake):**

```sh
nix profile install github:davidborzek/ringo#ringo-flow
```

To run scheduled monitors as a systemd service, use the
[NixOS module](nixos.md).

> Homebrew 6.0+ requires third-party taps to be trusted before use. If `brew
> install` prompts you to trust the tap, accept it — or trust it up front:
>
> ```sh
> brew tap davidborzek/tap
> brew trust --formula davidborzek/tap/ringo-flow
> ```

## Run a scenario

Credentials and the SIP domain come from the environment (via
[`env(...)`](js-api/functions/env.md)), so nothing sensitive lives in the script:

```sh
SIP_DOMAIN=example.com A_USER=alice A_PASS=… B_USER=bob B_PASS=… \
  ringo-flow run scenario.js
```

```sh
ringo-flow run scenario.js     # one file
ringo-flow run scenarios/      # a directory (all *.js, recursively)
ringo-flow check scenario.js   # syntax-check only (no SIP traffic)
```

The exit code is non-zero if any scenario fails.

The frontend follows the file extension, so there is nothing to configure:
`.js` runs on the JavaScript frontend, `.rhai` on the
[deprecated Rhai one](rhai.md).

## Editor support

Write the type definitions next to your scenarios once, and any editor with
TypeScript support checks the whole DSL as you type:

```sh
ringo-flow definitions --lang js ringo-flow.d.ts
```

```jsonc
// jsconfig.json — next to your scenarios
{
  "compilerOptions": {
    "checkJs": true,
    "strict": true,
    "noEmit": true,
    "target": "es2022",
    "module": "esnext",
    "types": []
  },
  "files": ["ringo-flow.d.ts", "scenario.js"]
}
```

Start each scenario with `// @ts-check` to get the same errors on the command
line via `tsc --noEmit`. For authoring in real TypeScript, see
[Writing scenarios](writing-scenarios.md#writing-scenarios-in-typescript).

## Useful flags

- `--scenario <pattern>` — run a subset by name (`re:` for a regex).
- `--tag <tag>` / `--exclude-tag <tag>` — filter by tag (repeatable, comma-separated).
- `--env-file FILE` — load variables for `env(...)` (a sibling `<scenario>.env`
  is layered on top per file).
- `--log [<file>]` — write the backend/SIP log to stderr (or a file); off by default.
- `--sip-trace [<file>]` — trace every SIP request/response to its own destination
  (stderr, or a file); separate from `--log`, off by default. A `.pcap` path writes
  a capture for sngrep/Wireshark — see [Debugging](debugging.md).
- `--save-audio` — save sent/received WAVs to the working directory.
- `--json` — emit NDJSON events (for CI).
- `-q` / `-v`, `--no-color`.

See `ringo-flow run --help` for the full list.
