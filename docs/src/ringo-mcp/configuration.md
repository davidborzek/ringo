# Configuration

The config is TOML: a list of `[[agent]]` tables plus the optional `[backend]`
and `[ringo]` tables — agents and how they run, nothing else. The path comes
from `--config <path>`, `$RINGO_MCP_CONFIG`, or defaults to
`~/.config/ringo-mcp/config.toml`. Runtime knobs are CLI flags instead:
`--enabled-tools` / `--disable` trim the tool surface and `--dial-deny` /
`--dial-allow` restrict dial targets (see [Getting started](getting-started.md)).

Everything is validated at server startup — unknown keys, duplicate names,
empty required fields, bad enum values and unusable password sources fail the
start with a clear error instead of surprising you mid-call.

## `[[agent]]`

One agent = one SIP identity = one SIP user agent in its own worker process.

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `profile` | string | — | Take the account (username, domain, credentials, custom headers, …) from this [ringo-phone profile](#agents-from-ringo-phone-profiles). Every other key set on the agent overrides the profile's value. |
| `name` | string | *required* | Unique label every MCP tool addresses the agent by. With `profile`: optional, defaults to the profile name's slug. |
| `username` | string | *required* | SIP username (user part of the AOR). With `profile`: from the profile unless set here. |
| `domain` | string | *required* | SIP domain/registrar (host part of the AOR). Bare dial targets resolve against it. With `profile`: from the profile unless set here. |
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
| `audio_codecs` | array | `[]` | Restrict/order offered codecs, most-preferred first, e.g. `["G722/16000/1", "PCMU"]`. Empty = the default codec set. |
| `custom_headers` | table / array | `[]` | Custom headers on outgoing INVITEs — see below. |
| `description` | string | — | Free-text note shown to the LLM: what the agent is for, how to reach it. Not taken from a profile. |
| `metadata` | table | `{}` | Free-form facts shown to the LLM, e.g. `number` (how other agents dial this one). Merged key by key over the profile's `[metadata]`. See [Telling the LLM how to reach agents](#telling-the-llm-how-to-reach-agents). |

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

### Telling the LLM how to reach agents

The SIP address of an agent is often not what you dial to reach it (a
provider's SIP usernames aren't phone numbers). `description` and `metadata`
are shown to the LLM in `list_agents`/`agent_status` and never interpreted by
ringo-mcp, so they can say what the agent is and how to call it:

```toml
[[agent]]
name = "front-desk"
username = "1001"
domain = "pbx.example.com"
password_cmd = "pass show sip/front-desk"
description = "Front desk, answers incoming calls"
metadata = { number = "+492111234567", extension = "21" }
```

The tool descriptions point the LLM at `metadata.number` for calling one agent
from another. For agents built from a ringo-phone profile, the profile's
`[metadata]` is used as is; keys set on the agent win.

## Agents from ringo-phone profiles

An agent can take its whole account — username, domain, credentials, custom
headers — from a [ringo-phone profile](../ringo-phone/profiles.md) instead of
repeating the fields. Per agent with `profile`, or in bulk with `[ringo]`
`profiles`:

```toml
[[agent]]
profile = "[prod] Office"      # account, credentials, custom_headers from here
name = "prod-office"            # optional; default: slug of the profile name
dtmf_mode = "info"              # any explicit field overrides the profile

[ringo]
# Every profile whose name matches becomes an agent (named by slug).
profiles = ['^\[test\]']
```

ringo-mcp only ever *reads* profiles — it never modifies them. Profiles are
resolved once at server startup, so a profile edited later needs a restart.

### Pattern rules

The regexes work like `--dial-allow`/`--dial-deny`: unanchored (they match
anywhere in the profile name), case-sensitive (prefix `(?i)` to ignore case).
They match against profile *names* — which typically contain `[`/`]`, and in a
regex those start a character class. Escape them, and prefer TOML literal
strings (single quotes), where backslashes need no doubling:

```toml
profiles = ['^[test]']    # WRONG: the character class [test] matches any name
                          # starting with t, e or s
profiles = ['^\[test\]']  # right: names starting with "[test]"
```

A pattern that matches nothing fails the startup with the list of available
profile names, as do an invalid regex and a `profile = "…"` that doesn't
exist. Each pattern's matches are logged to stderr at startup (profile names
only), which makes an accidental broad match visible. A profile already
referenced by an explicit `[[agent]] profile = …` is skipped — the explicit
entry wins. Two agents ending up with the same name (two entries, or an entry
colliding with a pattern's slug) fail the startup, naming both sources.

### Overrides

With `profile`, every field set on the `[[agent]]` replaces the profile's
value; the rest comes from the profile. Two special cases:

- Password: setting `password`, `password_file` or `password_cmd` on the agent
  replaces the profile's *whole* password source; otherwise the profile's
  source is resolved (its `cmd` > `file` > inline, like ringo-phone).
- `custom_headers` set on the agent replace the profile's; absent → the
  profile's.
- `metadata` is merged key by key: the profile's `[metadata]`, then the
  agent's keys on top. `description` comes from the agent only.

The default agent name for a profile is its slug: lowercase, ASCII
alphanumerics kept, every other run of characters collapsed to one `-`
(`"[test] Alice"` → `test-alice`). Phone-only profile settings
(`notify`, `auto_hold`, `deflect`, `header_display`, `notes`) are ignored.

### Safety

`profiles` selects *accounts*, not just names: a broad pattern (say `.`)
exposes every matching profile — including LIVE trunks — to the LLM as a
dialable agent. With a client whose permission gate allows all ringo tools
unconfirmed (e.g. Pi allowing the ringo tool group), that means real calls
the LLM can place on its own. Scope patterns narrowly (`'^\[test\]'`), and keep
`--dial-allow`/`--dial-deny` as the second gate on what may be dialed at all.

## `[backend]`

Applies to every agent. All keys optional; `[[agent]]` fields always win.

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `audio_driver` | string | `"aubridge"` | `aubridge` = headless (no sound hardware; `play` renders tones/files, received audio is captured in-process). `pipewire`/`pulse`/… for real audio. |
| `user_agent` | string | `ringo-mcp/<version>` | SIP `User-Agent` string. |
| `max_calls` | number | 4 | Max simultaneous calls per agent. |
| `hold_other_calls` | bool | `false` | Auto-hold the active call when another comes up. Off by default: the LLM keeps explicit control (`hold`/`resume`). |
| `local_timeout_s` | number | 120 | Outgoing-call ring timeout in seconds. |
| `sip_cafile` | string | — | Trust this CA bundle for TLS SIP. |
| `sip_capath` | string | — | Path to a TLS CA directory (`""` disables). |
| `record_audio` | bool | `false` | Record every call by default (sent + received, in-process): each call starts a fresh recording, armed for `save_audio` without `record_start`/`record: true`. |

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

Without any `profile`/`profiles` reference, ringo-mcp never touches your
ringo-phone profiles — the two configs stay fully independent (share
credentials via `password_cmd` if you want to).
