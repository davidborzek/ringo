//! The ringo-mcp config file (TOML): a list of `[[agent]]` tables plus an
//! optional `[backend]` table.
//!
//! ```toml
//! [[agent]]
//! name = "alice"
//! username = "1001"
//! domain = "pbx.example.com"
//! password = "secret"
//! # password_file = "~/secrets/alice"   # or password_cmd = "pass show sip/alice"
//!
//! [[agent]]
//! name = "bob"
//! username = "1002"
//! domain = "pbx.example.com"
//! dtmf_mode = "info"   # reliable DTMF when the audio source is idle
//!
//! [backend]
//! audio_driver = "aubridge"   # headless (default)
//! ```
//!
//! Agent names must be unique — every MCP tool addresses an agent by name.

use anyhow::{Context, Result, bail};
use ringo_core::account::{Account, BackendOptions};
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Command as OsCommand;

fn default_true() -> bool {
    true
}

/// One configured agent after loading: its account plus the custom-header
/// templates (raw, unresolved) for outgoing INVITEs.
#[derive(Debug)]
pub struct AgentDef {
    /// Unique label every MCP tool uses to address this agent.
    pub name: String,
    /// The SIP account to register.
    pub account: Account,
    /// Custom headers on outgoing INVITEs, `(name, template)` pairs.
    /// Templates may contain `${uuid}` (fresh per call); see `headers` module.
    pub custom_headers: Vec<(String, String)>,
}

/// One `[[agent]]` table: a flat, ergonomic mirror of a
/// [`ringo_core::account::Account`] plus the agent label.
#[derive(Debug, Deserialize)]
pub struct AgentEntry {
    /// Unique label every MCP tool uses to address this agent.
    pub name: String,
    /// SIP username (the user part of the AOR).
    pub username: String,
    /// SIP domain/registrar (the host part of the AOR).
    pub domain: String,
    /// SIP password (inline). Consider `password_file`/`password_cmd` instead.
    #[serde(default)]
    pub password: String,
    /// Read the SIP password from this file (a single trailing newline is
    /// stripped). Overrides `password`.
    #[serde(default)]
    pub password_file: Option<String>,
    /// Run this command via `sh -c`; its stdout is the SIP password (a single
    /// trailing newline is stripped). Overrides `password_file` and `password`.
    #[serde(default)]
    pub password_cmd: Option<String>,
    /// Display name for the AOR (`"Alice <sip:…>"`).
    pub display_name: Option<String>,
    /// SIP transport (`udp` / `tcp` / `tls` / `wss`).
    pub transport: Option<String>,
    /// Authentication username, if it differs from `username`.
    pub auth_user: Option<String>,
    /// Outbound proxy, e.g. `sip:proxy.example.com`.
    pub outbound: Option<String>,
    /// STUN server, e.g. `stun:stun.example.com`.
    pub stun_server: Option<String>,
    /// Media encryption (`srtp`, `zrtp`, `dtls_srtp`, …).
    pub media_enc: Option<String>,
    /// Re-registration interval in seconds; `0` disables registration.
    pub regint: Option<u32>,
    /// Subscribe to message-waiting indication.
    #[serde(default)]
    pub mwi: bool,
    /// DTMF transmission mode (`rtpevent` / `info` / `auto`). `info` sends DTMF
    /// as SIP INFO, which works even when the audio source is idle (headless
    /// agents that never call `play`).
    pub dtmf_mode: Option<String>,
    /// Accept incoming INVITEs addressed to identities other than the
    /// registration username (baresip `catchall`). Default on — each agent is
    /// the only UA in its own worker process, so the fallback is unambiguous
    /// (see the `Account::catchall` docs).
    #[serde(default = "default_true")]
    pub catchall: bool,
    /// Restrict/order the offered audio codecs, most-preferred first,
    /// e.g. `["opus", "PCMU"]`. Empty = baresip's default set/order.
    #[serde(default)]
    pub audio_codecs: Vec<String>,
    /// Custom headers on outgoing INVITEs, `[["Name", "value"], …]` pairs
    /// (or a `{ Name = "value" }` table). Values are templates: `${uuid}`
    /// expands to a fresh identifier per call, `$$` is a literal `$`.
    #[serde(default, deserialize_with = "deserialize_custom_headers")]
    pub custom_headers: Vec<(String, String)>,
}

/// The optional `[backend]` table: headless-sane defaults, overridable.
#[derive(Debug, Default, Deserialize)]
pub struct BackendEntry {
    /// Audio driver. Default `aubridge` (headless: no sound hardware, calls
    /// establish without a device; `play` renders tones/files, received audio
    /// is captured in-process). Set e.g. `pipewire` for real audio.
    pub audio_driver: Option<String>,
    /// SIP `User-Agent` string. Default `ringo-mcp/<version>`.
    pub user_agent: Option<String>,
    /// Max simultaneous calls per agent. `None` = baresip's default (4).
    pub max_calls: Option<u32>,
    /// Auto-hold the active call when another comes up. Default `false` — an
    /// LLM agent keeps explicit control over hold/resume (like ringo-flow).
    pub hold_other_calls: Option<bool>,
    /// Outgoing-call ring timeout in seconds. `None` = baresip's default.
    pub local_timeout_s: Option<u32>,
    /// Trust this CA bundle for TLS SIP.
    pub sip_cafile: Option<String>,
    /// Path to a TLS CA directory ("" to disable).
    pub sip_capath: Option<String>,
    /// Capture the full call's sent + received audio in-process (for
    /// `save_audio`). Off by default; only a short rolling window is retained.
    pub record_audio: Option<bool>,
}

/// The optional `[bridge]` table: the live-audio WebSocket bridge (see the
/// `bridge` module). Only the bind host is configurable — the port is always
/// ephemeral, and remote access belongs behind a reverse proxy, not an open
/// listener.
#[derive(Debug, Default, Deserialize)]
pub struct BridgeEntry {
    /// Host the WS bridge listens on. Default `127.0.0.1`.
    listen_host: Option<String>,
}

/// Validated bridge config.
#[derive(Debug, Clone, Copy)]
pub struct BridgeConfig {
    /// Loopback address the WS bridge binds to.
    pub listen_host: IpAddr,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            listen_host: IpAddr::from([127, 0, 0, 1]),
        }
    }
}

/// The parsed config file, ready to spawn agents from.
#[derive(Debug)]
pub struct LoadedConfig {
    /// Agent definitions in file order.
    pub agents: Vec<AgentDef>,
    /// Backend options with ringo-mcp defaults applied.
    pub backend: BackendOptions,
    /// The live-audio bridge config.
    pub bridge: BridgeConfig,
}

/// Top-level document of the TOML config file.
#[derive(Debug, Default, Deserialize)]
struct ConfigDoc {
    #[serde(default)]
    agent: Vec<AgentEntry>,
    #[serde(default)]
    backend: BackendEntry,
    #[serde(default)]
    bridge: BridgeEntry,
}

/// Default config path: `$RINGO_MCP_CONFIG`, else `~/.config/ringo-mcp/config.toml`.
pub fn default_path() -> PathBuf {
    if let Some(p) = std::env::var_os("RINGO_MCP_CONFIG") {
        return PathBuf::from(p);
    }
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => match std::env::var("HOME") {
            // No HOME → the path is only used for the error message anyway.
            Ok(h) if !h.is_empty() => PathBuf::from(h).join(".config"),
            _ => return PathBuf::from("ringo-mcp/config.toml"),
        },
    };
    base.join("ringo-mcp").join("config.toml")
}

/// Read and parse the config file at `path`, resolving passwords.
pub fn load(path: &Path) -> Result<LoadedConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read config file `{}`", path.display()))?;
    let doc: ConfigDoc =
        toml::from_str(&raw).with_context(|| format!("parse config `{}`", path.display()))?;

    if doc.agent.is_empty() {
        bail!(
            "config `{}` defines no [[agent]] entries; add at least one \
             (name, username, domain, password)",
            path.display()
        );
    }

    let mut names = HashSet::new();
    let mut agents = Vec::with_capacity(doc.agent.len());
    for mut entry in doc.agent {
        if entry.name.trim().is_empty() {
            bail!("agent `name` must not be empty");
        }
        if !names.insert(entry.name.clone()) {
            bail!("duplicate agent name `{}` in config", entry.name);
        }
        if entry.username.trim().is_empty() {
            bail!("agent `{}`: `username` must not be empty", entry.name);
        }
        if entry.domain.trim().is_empty() {
            bail!("agent `{}`: `domain` must not be empty", entry.name);
        }
        if let Some(mode) = entry.dtmf_mode.as_deref() {
            if !matches!(mode, "rtpevent" | "info" | "auto") {
                bail!(
                    "agent `{}`: `dtmf_mode` must be one of rtpevent/info/auto, got `{mode}`",
                    entry.name
                );
            }
        }
        let name = entry.name.clone();
        let agent_headers = std::mem::take(&mut entry.custom_headers);
        let account = build_account(entry)?;
        agents.push(AgentDef {
            name,
            custom_headers: agent_headers,
            account,
        });
    }

    let backend = build_backend(doc.backend);
    let bridge = build_bridge(doc.bridge, path)?;
    Ok(LoadedConfig {
        agents,
        backend,
        bridge,
    })
}

/// Convert one `[[agent]]` table into a ringo-core [`Account`], resolving the
/// password from `password_cmd` / `password_file` / `password`.
fn build_account(entry: AgentEntry) -> Result<Account> {
    let password = resolve_password(&entry)?;
    Ok(Account {
        username: entry.username,
        domain: entry.domain,
        password,
        display_name: entry.display_name,
        transport: entry.transport,
        auth_user: entry.auth_user,
        outbound: entry.outbound,
        stun_server: entry.stun_server,
        media_enc: entry.media_enc,
        regint: entry.regint,
        mwi: entry.mwi,
        dtmf_mode: entry.dtmf_mode,
        catchall: entry.catchall,
        audio_codecs: entry.audio_codecs,
    })
}

/// `password_cmd` > `password_file` > `password`.
fn resolve_password(entry: &AgentEntry) -> Result<String> {
    if let Some(cmd) = entry.password_cmd.as_deref().filter(|s| !s.is_empty()) {
        let out = OsCommand::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .with_context(|| format!("run password_cmd for agent `{}`", entry.name))?;
        if !out.status.success() {
            bail!(
                "password_cmd for agent `{}` exited with {}",
                entry.name,
                out.status
            );
        }
        return Ok(strip_one_newline(
            String::from_utf8_lossy(&out.stdout).into_owned(),
        ));
    }
    if let Some(file) = entry.password_file.as_deref().filter(|s| !s.is_empty()) {
        let p = expand_tilde(file);
        let raw = std::fs::read_to_string(&p).with_context(|| {
            format!(
                "read password_file `{}` (agent `{}`)",
                p.display(),
                entry.name
            )
        })?;
        return Ok(strip_one_newline(raw));
    }
    Ok(entry.password.clone())
}

/// Strip a single trailing newline (optionally `\r\n`), like ringo-phone.
fn strip_one_newline(mut s: String) -> String {
    if s.ends_with('\n') {
        s.pop();
        if s.ends_with('\r') {
            s.pop();
        }
    }
    s
}

/// Expand a leading `~/` to `$HOME`; other paths unchanged.
fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

/// Apply ringo-mcp defaults to the `[backend]` table.
fn build_backend(entry: BackendEntry) -> BackendOptions {
    BackendOptions {
        audio_driver: entry.audio_driver.or_else(|| Some("aubridge".into())),
        user_agent: entry
            .user_agent
            .or_else(|| Some(format!("ringo-mcp/{}", env!("CARGO_PKG_VERSION")))),
        hold_other_calls: entry.hold_other_calls.or(Some(false)),
        max_calls: entry.max_calls,
        local_timeout_s: entry.local_timeout_s,
        sip_cafile: entry.sip_cafile,
        sip_capath: entry.sip_capath,
        record_audio: entry.record_audio.unwrap_or(false),
        ..Default::default()
    }
}

/// Validate the `[bridge]` table: the listen host must be a loopback address.
/// Remote access goes through a reverse proxy in front of ringo-mcp (TLS +
/// auth), never through an open listener of ours.
fn build_bridge(entry: BridgeEntry, path: &Path) -> Result<BridgeConfig> {
    let Some(host) = entry.listen_host else {
        return Ok(BridgeConfig::default());
    };
    let ip: IpAddr = host.parse().with_context(|| {
        format!(
            "config `{}`: [bridge] listen_host must be an IP address, got `{host}`",
            path.display()
        )
    })?;
    if !ip.is_loopback() {
        bail!(
            "config `{}`: [bridge] listen_host `{host}` is not a loopback address \
             (v1 is localhost-only; put a reverse proxy in front for remote access)",
            path.display()
        );
    }
    Ok(BridgeConfig { listen_host: ip })
}

/// Accept custom headers both as an array of `[[key, value]]` pairs (the
/// multi-value form, order and duplicates preserved) and as a `{ key = value }`
/// table — the same two forms ringo-phone profiles accept.
fn deserialize_custom_headers<'de, D>(d: D) -> Result<Vec<(String, String)>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct V;

    impl<'de> serde::de::Visitor<'de> for V {
        type Value = Vec<(String, String)>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a table {key=value} or an array [[key, value], ...]")
        }

        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut out = Vec::with_capacity(map.size_hint().unwrap_or(0));
            while let Some(entry) = map.next_entry::<String, String>()? {
                out.push(entry);
            }
            Ok(out)
        }

        fn visit_seq<S: serde::de::SeqAccess<'de>>(
            self,
            mut seq: S,
        ) -> Result<Self::Value, S::Error> {
            let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
            while let Some(pair) = seq.next_element::<(String, String)>()? {
                out.push(pair);
            }
            Ok(out)
        }
    }

    d.deserialize_any(V)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_config(dir: &std::path::Path, body: &str) -> PathBuf {
        let p = dir.join("config.toml");
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn parses_two_agents_and_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let p = write_config(
            tmp.path(),
            r#"
[[agent]]
name = "alice"
username = "1001"
domain = "pbx.example.com"
password = "pw"

[[agent]]
name = "bob"
username = "1002"
domain = "other.example.com"
dtmf_mode = "info"
audio_codecs = ["opus", "PCMU"]
catchall = false
custom_headers = [["X-Session-Tag", "session-${uuid}"]]
"#,
        );
        let cfg = load(&p).unwrap();
        assert_eq!(cfg.agents.len(), 2);
        let alice = &cfg.agents[0];
        assert_eq!(alice.name, "alice");
        assert_eq!(alice.account.username, "1001");
        assert_eq!(alice.account.password, "pw");
        assert!(alice.account.catchall, "catchall defaults to true");

        let bob = &cfg.agents[1];
        assert_eq!(bob.account.dtmf_mode.as_deref(), Some("info"));
        assert!(!bob.account.catchall);
        assert_eq!(bob.account.audio_codecs, vec!["opus", "PCMU"]);
        assert_eq!(
            bob.custom_headers,
            vec![("X-Session-Tag".into(), "session-${uuid}".into())]
        );

        let b = &cfg.backend;
        assert_eq!(b.audio_driver.as_deref(), Some("aubridge"));
        assert_eq!(b.hold_other_calls, Some(false));
        assert!(b.user_agent.as_deref().unwrap().starts_with("ringo-mcp/"));
    }

    #[test]
    fn rejects_duplicate_names_and_empty_lists() {
        let tmp = tempfile::tempdir().unwrap();
        let p = write_config(
            tmp.path(),
            r#"
[[agent]]
name = "a"
username = "1"
domain = "x"
password = "pw"

[[agent]]
name = "a"
username = "2"
domain = "x"
password = "pw"
"#,
        );
        assert!(load(&p).unwrap_err().to_string().contains("duplicate"));

        let empty = write_config(tmp.path(), "# nothing\n");
        assert!(
            load(&empty)
                .unwrap_err()
                .to_string()
                .contains("no [[agent]]")
        );
    }

    #[test]
    fn password_file_beats_inline_and_strips_one_newline() {
        let tmp = tempfile::tempdir().unwrap();
        let secret = tmp.path().join("secret.txt");
        std::fs::write(&secret, "hunter2\n").unwrap();
        let p = write_config(
            tmp.path(),
            &format!(
                r#"
[[agent]]
name = "a"
username = "1"
domain = "x"
password = "inline"
password_file = "{}"
"#,
                secret.display()
            ),
        );
        let cfg = load(&p).unwrap();
        assert_eq!(cfg.agents[0].account.password, "hunter2");
    }

    #[test]
    fn bridge_listen_host_must_be_loopback() {
        let tmp = tempfile::tempdir().unwrap();
        let mk = |host: &str| {
            let p = tmp.path().join("b.toml");
            std::fs::write(
                &p,
                format!(
                    "[[agent]]\nname = \"a\"\nusername = \"1\"\ndomain = \"x\"\npassword = \"pw\"\n\n[bridge]\nlisten_host = \"{host}\"\n"
                ),
            )
            .unwrap();
            p
        };

        let cfg = load(&mk("127.0.0.1")).unwrap();
        assert_eq!(cfg.bridge.listen_host.to_string(), "127.0.0.1");
        let cfg = load(&mk("::1")).unwrap();
        assert_eq!(cfg.bridge.listen_host.to_string(), "::1");

        let err = load(&mk("0.0.0.0")).unwrap_err().to_string();
        assert!(err.contains("loopback"), "{err}");
        let err = load(&mk("192.168.1.5")).unwrap_err().to_string();
        assert!(err.contains("reverse proxy"), "{err}");

        // No [bridge] table → default loopback.
        let p = tmp.path().join("none.toml");
        std::fs::write(
            &p,
            "[[agent]]\nname = \"a\"\nusername = \"1\"\ndomain = \"x\"\npassword = \"pw\"\n",
        )
        .unwrap();
        assert_eq!(
            load(&p).unwrap().bridge.listen_host.to_string(),
            "127.0.0.1"
        );
    }

    #[test]
    fn custom_headers_accept_both_forms() {
        let tmp = tempfile::tempdir().unwrap();
        let p = write_config(
            tmp.path(),
            r#"
[[agent]]
name = "a"
username = "1"
domain = "x"
password = "pw"
custom_headers = { X-Foo = "bar", X-Baz = "qux" }
"#,
        );
        let cfg = load(&p).unwrap();
        // The table form has no guaranteed order (unlike the array form).
        let mut got = cfg.agents[0].custom_headers.clone();
        got.sort();
        assert_eq!(
            got,
            vec![
                ("X-Baz".into(), "qux".into()),
                ("X-Foo".into(), "bar".into())
            ]
        );
    }

    #[test]
    fn strip_one_newline_handles_crlf() {
        assert_eq!(strip_one_newline("ab\n".into()), "ab");
        assert_eq!(strip_one_newline("ab\r\n".into()), "ab");
        assert_eq!(strip_one_newline("ab\n\n".into()), "ab\n");
        assert_eq!(strip_one_newline("ab".into()), "ab");
    }

    #[test]
    fn validates_fields_at_load_time() {
        let tmp = tempfile::tempdir().unwrap();
        let mk = |body: &str| {
            let p = tmp.path().join(format!("c{}.toml", std::process::id()));
            std::fs::write(&p, body).unwrap();
            p
        };

        let empty_user = mk(r#"
[[agent]]
name = "a"
username = ""
domain = "x"
password = "pw"
"#);
        assert!(
            load(&empty_user)
                .unwrap_err()
                .to_string()
                .contains("username")
        );

        let empty_domain = mk(r#"
[[agent]]
name = "a"
username = "1"
domain = ""
password = "pw"
"#);
        assert!(
            load(&empty_domain)
                .unwrap_err()
                .to_string()
                .contains("domain")
        );

        let bad_dtmf = mk(r#"
[[agent]]
name = "a"
username = "1"
domain = "x"
password = "pw"
dtmf_mode = "smoke_signals"
"#);
        let err = load(&bad_dtmf).unwrap_err().to_string();
        assert!(err.contains("dtmf_mode"), "{err}");
        assert!(err.contains("smoke_signals"), "{err}");
    }
}
