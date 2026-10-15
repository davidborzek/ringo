//! The ringo-mcp config file (TOML): a list of `[[agent]]` tables plus the
//! optional `[backend]` and `[ringo]` tables.
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
//! profile = "[prod] Office"      # account, credentials, custom_headers from the profile
//! name = "prod-office"            # optional: defaults to the profile name's slug
//! dtmf_mode = "info"              # any explicit field overrides the profile
//!
//! [ringo]
//! # Every ringo-phone profile whose name matches becomes an agent (by slug).
//! profiles = ['^\\[test\\]']
//!
//! [backend]
//! audio_driver = "aubridge"   # headless (default)
//! ```
//!
//! Agent names must be unique — every MCP tool addresses an agent by name.

use anyhow::{Context, Result, bail};
use regex::Regex;
use ringo_core::account::{Account, BackendOptions};
use ringo_profile::profile::Profile;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command as OsCommand;

/// One configured agent after loading: its account plus the custom-header
/// templates (raw, unresolved) for outgoing INVITEs.
#[derive(Debug)]
pub struct AgentDef {
    /// Unique label every MCP tool uses to address this agent.
    pub name: String,
    /// The ringo-phone profile this agent was built from, if any; surfaced by
    /// `list_agents`/`agent_status` so a profile origin stays visible.
    pub profile: Option<String>,
    /// The SIP account to register.
    pub account: Account,
    /// Custom headers on outgoing INVITEs, `(name, template)` pairs.
    /// Templates may contain `${uuid}` (fresh per call); see
    /// `ringo_profile::header`.
    pub custom_headers: Vec<(String, String)>,
}

/// One `[[agent]]` table: a flat, ergonomic mirror of a
/// [`ringo_core::account::Account`], either complete in its own right or
/// built on a ringo-phone profile — every field set here overrides the
/// profile's value.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentEntry {
    /// Take the account (username, domain, credentials, custom headers, …)
    /// from this ringo-phone profile; any other field set on the agent
    /// overrides the profile's value.
    pub profile: Option<String>,
    /// Unique label every MCP tool uses to address this agent. Optional with
    /// `profile` — defaults to the profile name's slug.
    pub name: Option<String>,
    /// SIP username (the user part of the AOR). Optional with `profile`.
    pub username: Option<String>,
    /// SIP domain/registrar (the host part of the AOR). Optional with
    /// `profile`.
    pub domain: Option<String>,
    /// SIP password (inline). Consider `password_file`/`password_cmd` instead.
    #[serde(default)]
    pub password: Option<String>,
    /// Read the SIP password from this file (a single trailing newline is
    /// stripped). Overrides `password`. With `profile`: setting any password
    /// source on the agent replaces the profile's whole password source.
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
    pub mwi: Option<bool>,
    /// DTMF transmission mode (`rtpevent` / `info` / `auto`). `info` sends DTMF
    /// as SIP INFO, which works even when the audio source is idle (headless
    /// agents that never call `play`). Never taken from a profile.
    pub dtmf_mode: Option<String>,
    /// Accept incoming INVITEs addressed to identities other than the
    /// registration username (baresip `catchall`). Default on — each agent is
    /// the only UA in its own worker process, so the fallback is unambiguous
    /// (see the `Account::catchall` docs).
    #[serde(default)]
    pub catchall: Option<bool>,
    /// Restrict/order the offered audio codecs, most-preferred first,
    /// e.g. `["opus", "PCMU"]`. Empty = baresip's default set/order.
    #[serde(default)]
    pub audio_codecs: Option<Vec<String>>,
    /// Custom headers on outgoing INVITEs, `[["Name", "value"], …]` pairs
    /// (or a `{ Name = "value" }` table). Explicit headers replace the
    /// profile's. Values are templates: `${uuid}` expands to a fresh
    /// identifier per call, `$$` is a literal `$`.
    #[serde(default, deserialize_with = "deserialize_custom_headers")]
    pub custom_headers: Option<Vec<(String, String)>>,
}

/// The optional `[ringo]` table: ringo-phone integration.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RingoEntry {
    /// Turn every ringo-phone profile whose name matches one of these regexes
    /// into an agent (named by the profile name's slug). Unanchored,
    /// case-sensitive like the dial policy (`(?i)` to ignore case). Profiles
    /// referenced by an explicit `[[agent]] profile = …` are skipped.
    #[serde(default)]
    profiles: Vec<String>,
}

/// The optional `[backend]` table: headless-sane defaults, overridable.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// The parsed config file, ready to spawn agents from.
#[derive(Debug)]
pub struct LoadedConfig {
    /// Agent definitions in file order.
    pub agents: Vec<AgentDef>,
    /// Backend options with ringo-mcp defaults applied.
    pub backend: BackendOptions,
}

/// Top-level document of the TOML config file.
#[derive(Debug, Default, Deserialize)]
struct ConfigDoc {
    #[serde(default)]
    agent: Vec<AgentEntry>,
    #[serde(default)]
    backend: BackendEntry,
    #[serde(default)]
    ringo: RingoEntry,
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

/// Read and parse the config file at `path`, resolving profile references
/// against the user's ringo-phone profiles (`~/.config/ringo/profiles`).
pub fn load(path: &Path) -> Result<LoadedConfig> {
    load_resolved(path, None)
}

/// The seam behind [`load`]: `profiles_dir` resolves profile references
/// against that directory instead of the user's — ringo-profile locates the
/// real one via `$HOME`, which is process-global and off-limits to parallel
/// tests.
#[cfg(test)]
fn load_with_profiles_dir(path: &Path, profiles_dir: &Path) -> Result<LoadedConfig> {
    load_resolved(path, Some(profiles_dir))
}

fn load_resolved(path: &Path, profiles_dir: Option<&Path>) -> Result<LoadedConfig> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read config file `{}`", path.display()))?;
    let doc: ConfigDoc =
        toml::from_str(&raw).with_context(|| format!("parse config `{}`", path.display()))?;

    if doc.agent.is_empty() && doc.ringo.profiles.is_empty() {
        bail!(
            "config `{}` defines no [[agent]] entries; add at least one \
             (name, username, domain, password) or select ringo-phone profiles \
             via `[ringo] profiles`",
            path.display()
        );
    }

    let uses_profiles =
        !doc.ringo.profiles.is_empty() || doc.agent.iter().any(|a| a.profile.is_some());
    let available: Vec<String> = if uses_profiles {
        profile_names(profiles_dir)?
    } else {
        Vec::new()
    };
    // Profiles an explicit `[[agent]] profile = …` already uses: patterns skip
    // them (the explicit entry wins).
    let explicit: HashSet<String> = doc.agent.iter().filter_map(|a| a.profile.clone()).collect();

    // Final agent name → where it came from, for duplicate errors.
    let mut names: HashMap<String, String> = HashMap::new();
    let mut agents = Vec::with_capacity(doc.agent.len());

    for entry in doc.agent {
        let profile = match entry.profile.as_deref() {
            Some(pname) => {
                if !available.iter().any(|n| n.as_str() == pname) {
                    bail!(
                        "unknown profile `{pname}`; available profiles: {}",
                        fmt_names(&available)
                    );
                }
                Some((pname.to_string(), load_profile(pname, profiles_dir)?))
            }
            None => None,
        };
        let def = build_agent_def(entry, profile.as_ref().map(|(n, p)| (n.as_str(), p)))?;
        claim_name(&mut names, &def.name, "a `[[agent]]` entry".to_string())?;
        agents.push(def);
    }

    for pattern in &doc.ringo.profiles {
        let re = Regex::new(pattern)
            .with_context(|| format!("profiles pattern `{pattern}` is not a valid regex"))?;
        let matched: Vec<&String> = available.iter().filter(|n| re.is_match(n)).collect();
        if matched.is_empty() {
            bail!(
                "profiles pattern `{pattern}` matches no profile; available profiles: {}",
                fmt_names(&available)
            );
        }
        let (selected, skipped): (Vec<&str>, Vec<&str>) = matched
            .iter()
            .map(|n| n.as_str())
            .partition(|n| !explicit.contains(*n));
        eprintln!(
            "ringo-mcp: profiles pattern `{pattern}` selected: {}{}",
            if selected.is_empty() {
                "(none)".to_string()
            } else {
                selected.join(", ")
            },
            if skipped.is_empty() {
                String::new()
            } else {
                format!(" (skipped, explicit [[agent]]s: {})", skipped.join(", "))
            }
        );
        for pname in selected {
            let profile = load_profile(pname, profiles_dir)?;
            let entry = AgentEntry {
                profile: Some(pname.to_string()),
                ..AgentEntry::default()
            };
            let def = build_agent_def(entry, Some((pname, &profile)))?;
            claim_name(
                &mut names,
                &def.name,
                format!("profile `{pname}` (pattern `{pattern}`)"),
            )?;
            agents.push(def);
        }
    }

    let backend = build_backend(doc.backend);
    Ok(LoadedConfig { agents, backend })
}

/// Turn one `[[agent]]` table into an [`AgentDef`]: every field set on the
/// entry overrides the profile's value, the rest comes from the profile.
/// Without a profile the entry stands alone (classic form).
fn build_agent_def(entry: AgentEntry, profile: Option<(&str, &Profile)>) -> Result<AgentDef> {
    let profile_name = profile.map(|(name, _)| name);
    let profile = profile.map(|(_, p)| p);

    let name = match entry
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(n) => n.to_string(),
        None => match profile_name {
            Some(pname) => {
                let slug = profile_slug(pname);
                if slug.is_empty() {
                    bail!(
                        "profile `{pname}` yields an empty agent name (no alphanumeric \
                         characters in the profile name); set `name` explicitly"
                    );
                }
                slug
            }
            None => bail!("agent `name` must not be empty (or set `profile`)"),
        },
    };

    // Any password source on the entry replaces the profile's whole source;
    // otherwise the profile's own (cmd > file > inline) is resolved.
    // Computed before the field moves below — resolve_password borrows the
    // whole entry.
    let password = if entry.password.is_some()
        || entry
            .password_file
            .as_deref()
            .is_some_and(|s| !s.is_empty())
        || entry.password_cmd.as_deref().is_some_and(|s| !s.is_empty())
    {
        resolve_password(&entry, &name)?
    } else {
        match (profile_name, profile) {
            (Some(pname), Some(p)) => p.resolve_password().with_context(|| {
                format!("agent `{name}`: resolve password of profile `{pname}`")
            })?,
            _ => entry.password.clone().unwrap_or_default(),
        }
    };

    let username = entry
        .username
        .filter(|u| !u.trim().is_empty())
        .or_else(|| profile.map(|p| p.username.clone()))
        .unwrap_or_default();
    if username.trim().is_empty() {
        bail!("agent `{name}`: `username` must not be empty (or set `profile`)");
    }
    let domain = entry
        .domain
        .filter(|d| !d.trim().is_empty())
        .or_else(|| profile.map(|p| p.domain.clone()))
        .unwrap_or_default();
    if domain.trim().is_empty() {
        bail!("agent `{name}`: `domain` must not be empty (or set `profile`)");
    }

    if let Some(mode) = entry.dtmf_mode.as_deref() {
        if !matches!(mode, "rtpevent" | "info" | "auto") {
            bail!("agent `{name}`: `dtmf_mode` must be one of rtpevent/info/auto, got `{mode}`");
        }
    }

    let account = Account {
        username,
        domain,
        password,
        display_name: entry
            .display_name
            .or_else(|| profile.and_then(|p| p.display_name.clone())),
        transport: entry
            .transport
            .or_else(|| profile.and_then(|p| p.transport.clone())),
        auth_user: entry
            .auth_user
            .or_else(|| profile.and_then(|p| p.auth_user.clone())),
        outbound: entry
            .outbound
            .or_else(|| profile.and_then(|p| p.outbound.clone())),
        stun_server: entry
            .stun_server
            .or_else(|| profile.and_then(|p| p.stun_server.clone())),
        media_enc: entry
            .media_enc
            .or_else(|| profile.and_then(|p| p.media_enc.clone())),
        regint: entry.regint.or(profile.and_then(|p| p.regint)),
        mwi: entry.mwi.or(profile.map(|p| p.mwi)).unwrap_or(false),
        dtmf_mode: entry.dtmf_mode,
        catchall: entry
            .catchall
            .or(profile.map(|p| p.catchall))
            .unwrap_or(true),
        audio_codecs: entry
            .audio_codecs
            .or_else(|| profile.map(|p| p.audio_codecs.clone()))
            .unwrap_or_default(),
    };
    let custom_headers = entry
        .custom_headers
        .or_else(|| profile.map(|p| p.custom_headers.clone()))
        .unwrap_or_default();

    Ok(AgentDef {
        name,
        profile: profile_name.map(str::to_string),
        account,
        custom_headers,
    })
}

/// The default agent name for a profile: lowercase, ASCII alphanumerics kept,
/// every other run of characters collapsed to one `-`, `-` trimmed at the
/// ends ("[test] Alice" → `test-alice`).
fn profile_slug(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash {
            slug.push('-');
            dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

/// Record a final agent name; a second source for the same name is a startup
/// error naming both.
fn claim_name(names: &mut HashMap<String, String>, name: &str, source: String) -> Result<()> {
    use std::collections::hash_map::Entry;
    match names.entry(name.to_string()) {
        Entry::Occupied(first) => bail!(
            "duplicate agent name `{name}`: defined by both {} and {}",
            first.get(),
            source
        ),
        Entry::Vacant(v) => {
            v.insert(source);
            Ok(())
        }
    }
}

fn fmt_names(names: &[String]) -> String {
    if names.is_empty() {
        "(none)".into()
    } else {
        names.join(", ")
    }
}

/// The available profile names. The seam mirrors
/// `ringo_profile::profile::list_names` against an explicit dir for tests;
/// production asks ringo-profile (which locates the dir via `$HOME`).
fn profile_names(profiles_dir: Option<&Path>) -> Result<Vec<String>> {
    match profiles_dir {
        Some(dir) => list_profile_names_in(dir),
        None => ringo_profile::profile::list_names(),
    }
}

/// One profile by name; see [`profile_names`] for the seam.
fn load_profile(name: &str, profiles_dir: Option<&Path>) -> Result<Profile> {
    match profiles_dir {
        Some(dir) => load_profile_in(dir, name),
        None => ringo_profile::profile::load(name),
    }
}

fn list_profile_names_in(dir: &Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .with_context(|| format!("read profiles dir `{}`", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| e.path().join("profile.toml").exists())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    Ok(names)
}

fn load_profile_in(dir: &Path, name: &str) -> Result<Profile> {
    let path = dir.join(name).join("profile.toml");
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read profile `{name}`"))?;
    toml::from_str(&raw).with_context(|| format!("invalid profile.toml for `{name}`"))
}

/// `password_cmd` > `password_file` > `password` (the entry's own sources).
fn resolve_password(entry: &AgentEntry, name: &str) -> Result<String> {
    if let Some(cmd) = entry.password_cmd.as_deref().filter(|s| !s.is_empty()) {
        let out = OsCommand::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .with_context(|| format!("run password_cmd for agent `{name}`"))?;
        if !out.status.success() {
            bail!("password_cmd for agent `{name}` exited with {}", out.status);
        }
        return Ok(strip_one_newline(
            String::from_utf8_lossy(&out.stdout).into_owned(),
        ));
    }
    if let Some(file) = entry.password_file.as_deref().filter(|s| !s.is_empty()) {
        let p = expand_tilde(file);
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("read password_file `{}` (agent `{name}`)", p.display()))?;
        return Ok(strip_one_newline(raw));
    }
    Ok(entry.password.clone().unwrap_or_default())
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

/// Accept custom headers both as an array of `[[key, value]]` pairs (the
/// multi-value form, order and duplicates preserved) and as a `{ key = value }`
/// table — the same two forms ringo-phone profiles accept. Absent → `None`
/// (a profile may supply headers), present → `Some` (replaces the profile's).
fn deserialize_custom_headers<'de, D>(d: D) -> Result<Option<Vec<(String, String)>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct V;

    impl<'de> serde::de::Visitor<'de> for V {
        type Value = Option<Vec<(String, String)>>;

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
            Ok(Some(out))
        }

        fn visit_seq<S: serde::de::SeqAccess<'de>>(
            self,
            mut seq: S,
        ) -> Result<Self::Value, S::Error> {
            let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
            while let Some(pair) = seq.next_element::<(String, String)>()? {
                out.push(pair);
            }
            Ok(Some(out))
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
        assert_eq!(alice.profile, None, "classic agents have no profile");
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
    fn unknown_keys_are_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("unk.toml");
        std::fs::write(
            &p,
            "[[agent]]\nname = \"a\"\nusername = \"1\"\ndomain = \"x\"\npassword = \"pw\"\nusarname = \"typo\"\n",
        )
        .unwrap();
        // `{:#}` includes the serde cause under the parse-context layer.
        let err = format!("{:#}", load(&p).unwrap_err());
        assert!(err.contains("usarname"), "{err}");

        let p = tmp.path().join("unkb.toml");
        std::fs::write(&p, "[backend]\naudioo_driver = \"aubridge\"\n").unwrap();
        let err = format!("{:#}", load(&p).unwrap_err());
        assert!(err.contains("audioo_driver"), "{err}");

        let p = tmp.path().join("unkc.toml");
        std::fs::write(&p, "[ringo]\nprofiless = []\n").unwrap();
        let err = format!("{:#}", load(&p).unwrap_err());
        assert!(err.contains("profiless"), "{err}");
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

    // ─── ringo-phone profiles ─────────────────────────────────────────────

    /// A fake profiles dir, resolved through the test seam: names bracketed
    /// by purpose, both custom_headers forms, one password_file and one
    /// password_cmd profile. Never touches $HOME or the real profiles dir.
    fn fake_profiles(dir: &std::path::Path, secret_file: &std::path::Path) {
        std::fs::write(secret_file, "file-secret\n").unwrap();
        for (name, body) in [
            (
                "[test] Alice",
                r#"username = "2001"
domain = "pbx.example.com"
password = "test-secret"
display_name = "Test Alice"
custom_headers = [["X-Channel", "01"]]
"#
                .to_string(),
            ),
            (
                "[test] Bob",
                format!(
                    r#"username = "2002"
domain = "test.example.com"
password_file = "{}"
catchall = false
mwi = true
custom_headers = {{ X-Foo = "bar" }}
"#,
                    secret_file.display()
                ),
            ),
            (
                "[prod] Carol's Trunk",
                r#"username = "3001"
domain = "trunk.example.com"
password_cmd = "printf prod-secret"
transport = "tls"
"#
                .to_string(),
            ),
            (
                "Ops Plain",
                r#"username = "4001"
domain = "ops.example.com"
password = "ops-secret"
"#
                .to_string(),
            ),
        ] {
            let pdir = dir.join(name);
            std::fs::create_dir_all(&pdir).unwrap();
            std::fs::write(pdir.join("profile.toml"), body).unwrap();
        }
    }

    /// Config + fake profiles dir, loaded through the seam.
    fn load_with_profiles(tmp: &tempfile::TempDir, body: &str) -> Result<LoadedConfig> {
        let profiles = tmp.path().join("profiles");
        fake_profiles(&profiles, &tmp.path().join("secret.txt"));
        let p = write_config(tmp.path(), body);
        load_with_profiles_dir(&p, &profiles)
    }

    #[test]
    fn pattern_selects_profiles_by_regex() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = load_with_profiles(&tmp, "[ringo]\nprofiles = ['^\\[test\\]']\n").unwrap();
        assert_eq!(cfg.agents.len(), 2, "only the [test]-prefixed profiles");

        let a = &cfg.agents[0];
        assert_eq!(a.name, "test-alice", "slug of the profile name");
        assert_eq!(a.profile.as_deref(), Some("[test] Alice"));
        assert_eq!(a.account.username, "2001");
        assert_eq!(a.account.domain, "pbx.example.com");
        assert_eq!(a.account.password, "test-secret");
        assert_eq!(a.account.display_name.as_deref(), Some("Test Alice"));
        assert_eq!(a.custom_headers, vec![("X-Channel".into(), "01".into())]);
        assert!(a.account.catchall, "profile catchall default");
        assert!(a.account.mwi, "profile mwi defaults to on");
        assert_eq!(
            a.account.dtmf_mode, None,
            "MCP-only field, ringo-mcp default"
        );

        let b = &cfg.agents[1];
        assert_eq!(b.name, "test-bob");
        assert_eq!(b.account.password, "file-secret", "password_file resolved");
        assert!(!b.account.catchall, "explicit profile catchall");
        assert!(b.account.mwi, "explicit profile mwi");
        assert_eq!(b.custom_headers, vec![("X-Foo".into(), "bar".into())]);
    }

    #[test]
    fn pattern_matching_is_case_sensitive_and_unanchored() {
        let tmp = tempfile::tempdir().unwrap();
        // `(?i)` ignores case; the match is still the same set.
        let cfg = load_with_profiles(&tmp, "[ringo]\nprofiles = ['(?i)^\\[TEST\\]']\n").unwrap();
        assert_eq!(cfg.agents.len(), 2);

        // Case-sensitive without the flag: no `[TEST]` match, error with the list.
        let tmp = tempfile::tempdir().unwrap();
        let err = load_with_profiles(&tmp, "[ringo]\nprofiles = ['^\\[TEST\\]']\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("matches no profile"), "{err}");
        assert!(err.contains("[test] Alice"), "{err}");
        assert!(err.contains("Ops Plain"), "{err}");

        // Unanchored: matches anywhere in the name, not just at the start.
        let cfg = load_with_profiles(&tmp, "[ringo]\nprofiles = ['Plain']\n").unwrap();
        assert_eq!(cfg.agents.len(), 1);
        assert_eq!(cfg.agents[0].name, "ops-plain");
    }

    #[test]
    fn pattern_errors_name_the_cause() {
        let tmp = tempfile::tempdir().unwrap();
        let err = load_with_profiles(&tmp, "[ringo]\nprofiles = ['^\\[NOPE\\]']\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("matches no profile"), "{err}");
        assert!(err.contains("^\\[NOPE\\]"), "{err}");
        assert!(err.contains("Ops Plain"), "available names are listed");

        let err = load_with_profiles(&tmp, "[ringo]\nprofiles = ['[']\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a valid regex"), "{err}");
        assert!(err.contains("`[`"), "{err}");
    }

    #[test]
    fn unescaped_brackets_match_more_than_intended() {
        // `[test]` is the regex character class [test] — it matches any name
        // containing a t, e or s, not just the `[test]`-prefixed ones. The
        // escaped form selects exactly two; the pitfall pulls in every profile.
        let tmp = tempfile::tempdir().unwrap();
        let cfg = load_with_profiles(&tmp, "[ringo]\nprofiles = ['[test]']\n").unwrap();
        let names: Vec<&str> = cfg.agents.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["ops-plain", "prod-carol-s-trunk", "test-alice", "test-bob"]
        );
    }

    #[test]
    fn explicit_fields_override_the_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = load_with_profiles(
            &tmp,
            r#"
[[agent]]
profile = "[test] Alice"
name = "test-override"
dtmf_mode = "info"
transport = "tcp"
custom_headers = [["X-Override", "yes"]]
password_cmd = "printf overridden"
"#,
        )
        .unwrap();
        assert_eq!(cfg.agents.len(), 1);
        let a = &cfg.agents[0];
        assert_eq!(a.name, "test-override");
        assert_eq!(a.profile.as_deref(), Some("[test] Alice"));
        // Explicit wins…
        assert_eq!(a.account.dtmf_mode.as_deref(), Some("info"));
        assert_eq!(a.account.transport.as_deref(), Some("tcp"));
        assert_eq!(a.account.password, "overridden");
        assert_eq!(a.custom_headers, vec![("X-Override".into(), "yes".into())]);
        // …the rest comes from the profile.
        assert_eq!(a.account.username, "2001");
        assert_eq!(a.account.domain, "pbx.example.com");
        assert_eq!(a.account.display_name.as_deref(), Some("Test Alice"));
        assert!(a.account.mwi, "profile mwi (unset on the agent)");
        assert!(a.account.catchall, "profile catchall default");
    }

    #[test]
    fn profile_agent_defaults_to_slug_and_resolves_password_cmd() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = load_with_profiles(
            &tmp,
            r#"
[[agent]]
profile = "[prod] Carol's Trunk"
"#,
        )
        .unwrap();
        assert_eq!(cfg.agents.len(), 1);
        let a = &cfg.agents[0];
        assert_eq!(a.name, "prod-carol-s-trunk");
        assert_eq!(a.account.password, "prod-secret", "profile password_cmd");
        assert_eq!(a.account.transport.as_deref(), Some("tls"));
    }

    #[test]
    fn explicit_profile_agents_win_over_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = load_with_profiles(
            &tmp,
            "[ringo]\nprofiles = ['^\\[test\\]']\n\
[[agent]]\nprofile = \"[test] Alice\"\nname = \"test-01\"\n",
        )
        .unwrap();
        let names: Vec<&str> = cfg.agents.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["test-01", "test-bob"], "explicit entry wins");
    }

    #[test]
    fn name_collisions_from_any_source_are_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let err = load_with_profiles(
            &tmp,
            "[[agent]]\nname = \"test-alice\"\nusername = \"1\"\ndomain = \"x\"\npassword = \"pw\"\n\
[ringo]\nprofiles = ['^\\[test\\]']\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("duplicate agent name `test-alice`"), "{err}");
        assert!(err.contains("a `[[agent]]` entry"), "{err}");
        assert!(
            err.contains("profile `[test] Alice` (pattern `^\\[test\\]`)"),
            "{err}"
        );

        // Two patterns matching the same profile collide the same way.
        let tmp = tempfile::tempdir().unwrap();
        let err = load_with_profiles(&tmp, "[ringo]\nprofiles = ['^\\[test\\] Ali', 'Alice']\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("duplicate agent name `test-alice`"), "{err}");
        assert!(err.contains("pattern `^\\[test\\] Ali`"), "{err}");
        assert!(err.contains("pattern `Alice`"), "{err}");
    }

    #[test]
    fn unknown_profile_is_an_error_listing_available() {
        let tmp = tempfile::tempdir().unwrap();
        let err = load_with_profiles(&tmp, "[[agent]]\nprofile = \"Nope\"\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown profile `Nope`"), "{err}");
        assert!(err.contains("[test] Alice"), "{err}");
        assert!(err.contains("Ops Plain"), "{err}");
    }

    #[test]
    fn slug_cases_and_the_empty_slug_error() {
        assert_eq!(profile_slug("[test] Alice"), "test-alice");
        assert_eq!(
            profile_slug("[prod EU] Carol's Trunk"),
            "prod-eu-carol-s-trunk"
        );
        assert_eq!(profile_slug("Ops---Plain 42"), "ops-plain-42");
        assert_eq!(profile_slug("Ünïcode"), "n-code");
        assert_eq!(profile_slug("---"), "");

        // A profile whose name has no alphanumeric characters cannot be
        // named by slug — error, not a silent empty agent name.
        let tmp = tempfile::tempdir().unwrap();
        let profiles = tmp.path().join("profiles");
        fake_profiles(&profiles, &tmp.path().join("secret.txt"));
        let pdir = profiles.join("---");
        std::fs::create_dir_all(&pdir).unwrap();
        std::fs::write(
            pdir.join("profile.toml"),
            "username = \"1\"\ndomain = \"x\"\npassword = \"pw\"\n",
        )
        .unwrap();
        let p = write_config(tmp.path(), "[[agent]]\nprofile = \"---\"\n");
        let err = load_with_profiles_dir(&p, &profiles)
            .unwrap_err()
            .to_string();
        assert!(err.contains("empty agent name"), "{err}");
        assert!(err.contains("`---`"), "{err}");
    }

    #[test]
    fn profile_without_ringo_table_never_touches_the_profiles_dir() {
        // A classic config must not fail because $HOME or the profiles dir is
        // missing/odd — profiles are only consulted when referenced.
        let tmp = tempfile::tempdir().unwrap();
        let p = write_config(
            tmp.path(),
            r#"
[[agent]]
name = "a"
username = "1"
domain = "x"
password = "pw"
"#,
        );
        let cfg = load_with_profiles_dir(&p, &tmp.path().join("does-not-exist")).unwrap();
        assert_eq!(cfg.agents.len(), 1);
        assert_eq!(cfg.agents[0].profile, None);
    }
}
