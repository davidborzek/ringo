# Demo GIF

`docs/src/assets/ringo-phone.gif` (shown in the README and the docs) is
recorded with [VHS](https://github.com/charmbracelet/vhs). To re-record it:

```sh
docs/demo/record.sh
```

The script builds `ringo` from this checkout, starts a throwaway
[Kamailio](https://www.kamailio.org) registrar and two headless callees (bob and
charlie, each answering 2 s after the call starts ringing), then records alice's
session. Everything runs on loopback; scratch state goes to
`~/.cache/ringo-demo-gif` and is recreated each run. Kamailio, VHS, ttyd and
ffmpeg come from nixpkgs, so only nix is needed.

| File | What it is |
|------|------------|
| `ringo.tape.in` | What happens on screen: keys and timings |
| `record.sh` | Sets up registrar, identities and callees, records, cleans up |
| `kamailio.cfg.in` | The demo registrar (in-memory, no auth — loopback only) |
| `theme.toml` | ringo colours used in the recording |
