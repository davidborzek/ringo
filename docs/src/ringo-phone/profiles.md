# Profiles

Each SIP account is a **profile**, stored as TOML at
`~/.config/ringo/profiles/<name>/profile.toml`. Create and edit profiles right in
the [picker](tui.md) (`Ctrl+N` / `Ctrl+E`), or write the file by hand:

```toml
username     = "user123"
password     = "secret"
domain       = "sip.example.com"
display_name = "My Name"               # optional
transport    = "tls"                   # optional: udp, tcp, tls
outbound     = "sip:proxy.example.com" # optional
stun_server  = "stun:stun.example.com" # optional
media_enc    = "dtls_srtp"             # optional
notify       = true                    # desktop notifications (default: true)
mwi          = true                    # message-waiting indicator (default: true)
```

## Custom SIP headers

Add headers to every outgoing INVITE. Order is preserved and duplicate keys are
allowed (e.g. RFC 4244 `History-Info`). Values are percent-encoded for baresip's
`uaaddheader` — write them as plain text, no manual escaping. The `${uuid}`
placeholder is replaced per call with a fresh UUIDv4 (shared across all headers in
the same INVITE); use `$$` for a literal `$`.

```toml
custom_headers = [
  ["History-Info", "<sip:1@example.com>;index=1"],
  ["History-Info", "<sip:2@example.com>;index=1.1"],
  ["X-Trace-Id",   "call-${uuid}"],
]
```

## File locations

| Path | Description |
|------|-------------|
| `~/.config/ringo/ringo.toml` | Global [config](configuration.md) |
| `~/.config/ringo/contacts.toml` | Contact book |
| `~/.config/ringo/profiles/<name>/profile.toml` | Profile config |
| `~/.config/ringo/profiles/<name>/call_history` | Per-profile call history (JSONL) |
| `~/.local/share/ringo/history` | Global dial history |
| `/tmp/ringo-<name>-<ts>/` | Runtime temp dir (auto-cleaned) |
| `/tmp/ringo-<name>.log` | Application log (hooks, TCP errors, lifecycle) |
