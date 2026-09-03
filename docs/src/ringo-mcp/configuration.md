# Configuration

The config is TOML: a list of `[[agent]]` tables plus one optional `[backend]`
table. The path comes from `--config <path>`, `$RINGO_MCP_CONFIG`, or defaults
to `~/.config/ringo-mcp/config.toml`.

Everything is validated at server startup — unknown keys, duplicate names,
empty required fields, bad enum values and unusable password sources fail the
start with a clear error instead of surprising you mid-call.

## `[[agent]]`

One agent = one SIP identity = one baresip UA in its own worker process.

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `name` | string | *required* | Unique label every MCP tool addresses the agent by. |
| `username` | string | *required* | SIP username (user part of the AOR). |
| `domain` | string | *required* | SIP domain/registrar (host part of the AOR). Bare dial targets resolve against it. |
| `password` | string | `""` | SIP password. Consider `password_file`/`password_cmd`. |
| `password_file` | string | — | Read the password from this file (one trailing newline stripped). Overrides `password`. A leading `~/` expands to `$HOME`. |
| `password_cmd` | string | — | Run via `sh -c`; stdout is the password (one trailing newline stripped). Overrides `password_file`. |
| `display_name` | string | — | Display name for the AOR (`"Alice <sip:…>"`). |
| `transport` | string | — | `udp` / `tcp` / `tls` / `wss`. |
| `auth_user` | string | — | Auth username, if it differs from `username`. |
| `outbound` | string | — | Outbound proxy, e.g. `sip:proxy.example.com;transport=tls`. |
| `stun_server` | string | — | STUN server, e.g. `stun:stun.example.net`. |
| `media_enc` | string | — | Media encryption: `srtp`, `zrtp`, `dtls_srtp`, `srtp-mand`, … |
| `regint` | number | — | Re-registration interval in seconds; `0` disables registration. |
| `mwi` | bool | `false` | Subscribe to message-waiting indication. |
| `dtmf_mode` | string | — | `rtpevent` / `info` / `auto`. `info` (SIP INFO) is the reliable choice when the agent never transmits audio. |
| `catchall` | bool | `true` | Accept INVITEs addressed to identities other than the registration username. Safe here: each worker is the only UA in its process. |
| `audio_codecs` | array | `[]` | Restrict/order offered codecs, most-preferred first, e.g. `["G722/16000/1", "PCMU"]`. Empty = baresip's defaults. |
| `custom_headers` | table / array | `[]` | Custom headers on outgoing INVITEs — see below. |

### Password resolution

`password_cmd` beats `password_file` beats `password`. Reading from a secret
manager keeps the credential out of the config file; pointing `password_cmd`
at another config that already has the password (e.g. a ringo-phone profile)
keeps a single source of truth:

```toml
[[agent]]
name = "channel-01"
username = "1001"
domain = "pbx.example.com"
password_cmd = "grep '^password' '~/.config/ringo/profiles/acme.toml' | cut -d'\"' -f2"
```

### Custom headers

`custom_headers` accepts a table or an array of `[[name, value]]` pairs (the
array form preserves order and duplicates):

```toml
custom_headers = [
    ["History-Info", "<sip:1002@pbx.example.com>;index=1"],
    ["X-Session-Tag", "session-${uuid}"],
]
```

Values are templates, evaluated per outgoing call:

| Syntax | Expands to |
| ------ | ----------- |
| `${uuid}` | A fresh UUID v4 — **re-rendered on every outgoing call** (one value per call: multiple headers sharing the placeholder see the same UUID). |
| `$$` | A literal `$`. |
| anything else | Verbatim. |

Static templates (no placeholders) are set once when the agent starts;
dynamic ones are re-rendered by every `dial`. The same semantics as
[ringo-phone profiles](../ringo-phone/profiles.md).

## `[backend]`

Applies to every agent. All keys optional; `[[agent]]` fields always win.

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `audio_driver` | string | `"aubridge"` | `aubridge` = headless (no sound hardware; `play` renders tones/files, received audio is captured in-process). `pipewire`/`pulse`/… for real audio. |
| `user_agent` | string | `ringo-mcp/<version>` | SIP `User-Agent` string. |
| `max_calls` | number | baresip default (4) | Max simultaneous calls per agent. |
| `hold_other_calls` | bool | `false` | Auto-hold the active call when another comes up. Off by default: the LLM keeps explicit control (`hold`/`resume`). |
| `local_timeout_s` | number | baresip default (120) | Outgoing-call ring timeout in seconds. |
| `sip_cafile` | string | — | Trust this CA bundle for TLS SIP. |
| `sip_capath` | string | — | Path to a TLS CA directory (`""` disables). |
| `record_audio` | bool | `false` | Capture full sent+received audio in-process, for the `save_audio` tool. |

## A full example

```toml
[[agent]]
name = "alice"
username = "1001"
domain = "pbx.example.com"
password_cmd = "pass show sip/alice"
transport = "tls"
outbound = "sip:proxy.pbx.example.com;transport=tls"
media_enc = "srtp-mand"
audio_codecs = ["G722/16000/1"]
regint = 300
custom_headers = [["X-Session-Tag", "session-${uuid}"]]

[[agent]]
name = "bob"
username = "1002"
domain = "pbx.example.com"
password_file = "~/secrets/bob"
dtmf_mode = "info"

[backend]
audio_driver = "aubridge"
record_audio = true
```

ringo-mcp never reuses or rewrites your ringo-phone profiles — the two configs
are fully independent (share credentials via `password_cmd` if you want to).
