# Remote control

Drive a running session from another terminal — or a script — over a per-session
Unix socket. `ctl` is an alias for `control`.

```sh
ringo control list                   # running sessions: PID, profile, account
ringo control -t <target> <command> [args]

# examples
ringo control -t work dial 4711      # target by profile name
ringo control -t 215709 hangup       # ...or by PID
ringo control -t work dtmf 123#      # send DTMF into the active call
ringo control -t work status         # registration + active calls
```

`<target>` is a profile name or a **PID** — use the PID (from `ringo control list`)
when a name is awkward to type or the same profile runs more than once.

Commands: `dial <n>`, `hangup`, `accept`, `hold`, `resume`, `mute`,
`dtmf <digits>`, `transfer <uri>`, `status`, `shutdown`.

## Headless sessions

For scripting and automated testing, run a session without the TUI — it still
binds the control socket and registers, so you drive it entirely via
`ringo control`:

```sh
ringo start --headless work &    # runs in the background, no terminal needed
ringo control -t work status     # …drive it…
ringo control -t work shutdown   # stop it cleanly (or Ctrl-C the process)
```

## JSON output

Add `--json` (`-j`) for machine-readable output: `list` emits an array of sessions,
`status` a structured object (registration, active `calls`, and the most recently
closed call under `last_call` with its reason/duration), and every other command an
`{ "ok", "data", "error" }` envelope. The exit code reflects success.

```sh
ringo control list --json
ringo control -t work status --json
ringo control -t work dial 4711 --json   # {"ok":true,"data":"Dialing 4711","error":null}
```

> For full Rhai-scripted telephony test scenarios (multiple agents, assertions,
> audio verification), see [ringo-flow](../ringo-flow/introduction.md).
