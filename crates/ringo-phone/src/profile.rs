use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, fmt, fs, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Profile {
    pub display_name: Option<String>,
    pub username: String,
    pub auth_user: Option<String>,
    #[serde(default)]
    pub password: String,
    /// Read the SIP password from this file instead of `password` (a single
    /// trailing newline is stripped). Overrides `password`.
    #[serde(default)]
    pub password_file: Option<String>,
    /// Run this command (via `sh -c`); its stdout is the SIP password (a single
    /// trailing newline is stripped). Overrides `password_file` and `password`.
    #[serde(default)]
    pub password_cmd: Option<String>,
    pub domain: String,
    pub transport: Option<String>,
    pub outbound: Option<String>,
    pub stun_server: Option<String>,
    pub media_enc: Option<String>,
    /// Restrict/order the offered audio codecs (most-preferred first), e.g.
    /// `["opus", "PCMU"]`. Empty = baresip's default set/order.
    #[serde(default)]
    pub audio_codecs: Vec<String>,
    pub regint: Option<u32>,
    pub notes: Option<String>,
    #[serde(default, deserialize_with = "deserialize_custom_headers")]
    pub custom_headers: Vec<(String, String)>,
    /// Inbound-header views for incoming calls: `(label, template)` pairs. The
    /// template may reference received SIP headers via `${Header-Name}`, e.g.
    /// `("Debug", "http://debugtool/${X-Call-Sid}")`. Opt-in — empty shows
    /// nothing. Views whose referenced headers are absent on a call are skipped.
    #[serde(default, deserialize_with = "deserialize_custom_headers")]
    pub header_display: Vec<(String, String)>,
    #[serde(default = "default_true")]
    pub notify: bool,
    #[serde(default = "default_true")]
    pub mwi: bool,
    /// Automatically put the current call on hold when placing another call or
    /// switching lines (like baresip `call_hold_other_calls`). Default on; set
    /// to `false` to keep multiple calls active in parallel.
    #[serde(default = "default_true")]
    pub auto_hold: bool,
    /// Register as a baresip `catchall` UA so incoming INVITEs to identities
    /// other than the registration username are accepted instead of rejected
    /// with `404 (UA not found)`. On by default; ringo-phone runs a single UA.
    #[serde(default = "default_true")]
    pub catchall: bool,
    /// Deflect incoming calls with a 302 Moved Temporarily to `deflect_target`.
    /// The target is either a full SIP URI or a bare number/extension (resolved
    /// to `sip:<target>@<domain>` at startup, like ringo-flow's `deflect_to_uri`).
    #[serde(default)]
    pub deflect: bool,
    #[serde(default)]
    pub deflect_target: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            display_name: None,
            username: String::new(),
            auth_user: None,
            password: String::new(),
            password_file: None,
            password_cmd: None,
            domain: String::new(),
            transport: None,
            outbound: None,
            stun_server: None,
            media_enc: None,
            audio_codecs: Vec::new(),
            regint: None,
            notes: None,
            custom_headers: Vec::new(),
            header_display: Vec::new(),
            notify: false,
            mwi: false,
            auto_hold: true,
            catchall: true,
            deflect: false,
            deflect_target: None,
            metadata: HashMap::new(),
        }
    }
}

/// Accepts both the legacy table form (`[custom_headers] X-Foo = "bar"`)
/// and the multi-value array form (`[["X-Foo","bar"], ["X-Foo","baz"]]`).
fn deserialize_custom_headers<'de, D>(d: D) -> Result<Vec<(String, String)>, D::Error>
where
    D: Deserializer<'de>,
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

impl Profile {
    /// The SIP AOR string for this profile.
    pub fn aor(&self) -> String {
        format!("sip:{}@{}", self.username, self.domain)
    }

    /// Resolve the effective SIP password.
    ///
    /// Precedence: `password_cmd` (its stdout), then `password_file` (its
    /// contents), then the inline `password`. A single trailing newline (CRLF
    /// or LF) from a file or command is stripped; other characters are
    /// preserved. The command runs via `sh -c`.
    pub fn resolve_password(&self) -> Result<String> {
        if let Some(cmd) = self
            .password_cmd
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let out = std::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .with_context(|| format!("running password_cmd `{cmd}`"))?;
            if !out.status.success() {
                anyhow::bail!("password_cmd `{cmd}` failed with {}", out.status);
            }
            return Ok(strip_one_newline(&String::from_utf8_lossy(&out.stdout)));
        }
        if let Some(file) = self
            .password_file
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let path = expand_tilde(file);
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("reading password_file `{}`", path.display()))?;
            return Ok(strip_one_newline(&raw));
        }
        Ok(self.password.clone())
    }
}

/// Strip a single trailing newline (`\n` or `\r\n`), leaving other content
/// untouched — secret files and command output usually end in exactly one.
fn strip_one_newline(s: &str) -> String {
    let s = s.strip_suffix('\n').unwrap_or(s);
    let s = s.strip_suffix('\r').unwrap_or(s);
    s.to_string()
}

/// Expand a leading `~/` to `$HOME`; other paths are returned unchanged.
fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

// ─── Paths ───────────────────────────────────────────────────────────────────

pub fn profiles_dir() -> Result<PathBuf> {
    let base = dirs_base()?;
    Ok(base.join("profiles"))
}

pub fn dirs_base() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(".config").join("ringo"))
}

/// XDG state dir for ringo (`$XDG_STATE_HOME/ringo`, fallback `~/.local/state/ringo`)
/// — the proper place for the backend log (not /tmp).
pub fn state_dir() -> Result<PathBuf> {
    let base = match std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        Some(p) => p,
        None => {
            let home = std::env::var("HOME").context("HOME not set")?;
            PathBuf::from(home).join(".local").join("state")
        }
    };
    Ok(base.join("ringo"))
}

pub fn profile_dir(name: &str) -> Result<PathBuf> {
    Ok(profiles_dir()?.join(name))
}

// ─── Recently used ───────────────────────────────────────────────────────────

/// How many names to remember. Long enough that the profiles you actually work
/// with stay at hand, short enough that the file stays a list you could read.
const MAX_RECENT: usize = 20;

/// Where the order lives. In the state directory rather than the config: it is
/// something ringo observes about you, not something you configure, and it must
/// not turn up in a dotfile repo as a diff after every call.
fn recent_path() -> Result<PathBuf> {
    Ok(state_dir()?.join("recent"))
}

/// Profile names most recently started first. Unknown or deleted profiles are
/// not filtered here — [`order_by_recent`] ignores anything it does not find.
pub fn recent() -> Vec<String> {
    let Ok(path) = recent_path() else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .take(MAX_RECENT)
        .collect()
}

/// Move `name` to the front of the recently-used order.
///
/// Best effort: a state directory that cannot be written costs the ordering,
/// nothing else, so this reports no error. One name per line — the format is a
/// list, and a list is what a plain text file already is.
pub fn record_recent(name: &str) {
    let mut names = recent();
    names.retain(|n| n != name);
    names.insert(0, name.to_string());
    names.truncate(MAX_RECENT);

    let Ok(path) = recent_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, names.join("\n") + "\n");
}

/// `names` with the recently used ones first, in that order, and everything
/// else after them untouched — so the rest stays alphabetical and a profile
/// does not move around for reasons the user cannot see.
pub fn order_by_recent(names: Vec<String>, recent: &[String]) -> Vec<String> {
    // Deduplicated on the way in: record_recent never writes a name twice, but
    // the file is plain text somebody may have edited, and a duplicate there
    // would list the profile twice.
    let mut front: Vec<String> = Vec::new();
    for name in recent {
        if names.contains(name) && !front.contains(name) {
            front.push(name.clone());
        }
    }
    let rest = names.into_iter().filter(|n| !front.contains(n));
    front.iter().cloned().chain(rest).collect()
}

// ─── List ────────────────────────────────────────────────────────────────────

pub fn list_names() -> Result<Vec<String>> {
    let dir = profiles_dir()?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names: Vec<String> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| e.path().join("profile.toml").exists())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    Ok(names)
}

pub fn list(plain: bool, format: Option<String>, json: bool) -> Result<()> {
    let names = list_names()?;

    if json {
        let entries: Vec<serde_json::Value> = names
            .iter()
            .map(|name| {
                let p = load(name)?;
                let mut obj = serde_json::to_value(&p)?;
                let map = obj.as_object_mut().unwrap();
                map.insert("name".into(), serde_json::json!(name));
                map.insert("aor".into(), serde_json::json!(p.aor()));
                map.remove("password");
                Ok(obj)
            })
            .collect::<Result<_>>()?;
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if let Some(fmt) = &format {
        for name in &names {
            let p = load(name)?;
            println!("{}", format_profile(name, &p, fmt));
        }
        return Ok(());
    }

    if plain {
        for name in &names {
            println!("{}", name);
        }
        return Ok(());
    }

    if names.is_empty() {
        println!("No profiles found. Run: ringo");
        return Ok(());
    }
    println!("Profiles ({}):", names.len());
    for name in &names {
        let p = load(name)?;
        let transport = p.transport.as_deref().unwrap_or("default");
        println!(
            "  {:20}  {}@{}  [{}]",
            name, p.username, p.domain, transport
        );
    }
    Ok(())
}

fn format_profile(name: &str, profile: &Profile, fmt: &str) -> String {
    fmt.replace("{name}", name)
        .replace("{username}", &profile.username)
        .replace("{domain}", &profile.domain)
        .replace("{aor}", &profile.aor())
        .replace(
            "{display_name}",
            profile.display_name.as_deref().unwrap_or(""),
        )
        .replace(
            "{transport}",
            profile.transport.as_deref().unwrap_or("default"),
        )
        .replace("{auth_user}", profile.auth_user.as_deref().unwrap_or(""))
        .replace("{outbound}", profile.outbound.as_deref().unwrap_or(""))
        .replace(
            "{stun_server}",
            profile.stun_server.as_deref().unwrap_or(""),
        )
        .replace(
            "{media_enc}",
            profile.media_enc.as_deref().unwrap_or("none"),
        )
        .replace("{notes}", profile.notes.as_deref().unwrap_or(""))
}

// ─── Load / Save ─────────────────────────────────────────────────────────────

pub fn load(name: &str) -> Result<Profile> {
    let path = profile_dir(name)?.join("profile.toml");
    let raw =
        fs::read_to_string(&path).with_context(|| format!("Cannot read profile '{}'", name))?;
    toml::from_str(&raw).with_context(|| format!("Invalid profile.toml for '{}'", name))
}

pub fn rename(old: &str, new: &str) -> Result<()> {
    let from = profile_dir(old)?;
    let to = profile_dir(new)?;
    fs::rename(&from, &to).with_context(|| format!("Cannot rename '{}' to '{}'", old, new))
}

pub fn save(name: &str, profile: &Profile) -> Result<()> {
    let dir = profile_dir(name)?;
    fs::create_dir_all(&dir)?;

    let toml_str = toml::to_string_pretty(profile)?;
    fs::write(dir.join("profile.toml"), toml_str)?;

    Ok(())
}

#[cfg(test)]
mod recent_tests {
    use super::order_by_recent;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn recent_names_come_first_in_their_own_order() {
        let names = v(&["alpha", "beta", "gamma"]);
        let recent = v(&["gamma", "alpha"]);
        assert_eq!(
            order_by_recent(names, &recent),
            v(&["gamma", "alpha", "beta"])
        );
    }

    #[test]
    fn the_rest_keeps_the_order_it_came_in() {
        // list_names sorts; whatever is not recent must stay alphabetical
        // rather than shuffling for reasons the user cannot see.
        let names = v(&["alpha", "beta", "gamma", "delta"]);
        let recent = v(&["gamma"]);
        assert_eq!(
            order_by_recent(names, &recent),
            v(&["gamma", "alpha", "beta", "delta"])
        );
    }

    #[test]
    fn deleted_profiles_in_the_history_are_ignored() {
        // The file outlives the profiles it names.
        let names = v(&["alpha", "beta"]);
        let recent = v(&["long-gone", "beta"]);
        assert_eq!(order_by_recent(names, &recent), v(&["beta", "alpha"]));
    }

    #[test]
    fn no_history_changes_nothing() {
        let names = v(&["alpha", "beta"]);
        assert_eq!(order_by_recent(names.clone(), &[]), names);
    }

    #[test]
    fn a_name_listed_twice_appears_once() {
        // record_recent dedupes, but the file is plain text a user may edit.
        let names = v(&["alpha", "beta"]);
        let recent = v(&["beta", "beta"]);
        assert_eq!(order_by_recent(names, &recent), v(&["beta", "alpha"]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#"
        username = "u"
        password = "p"
        domain   = "d"
    "#;

    fn parse(extra: &str) -> Profile {
        let raw = format!("{BASE}\n{extra}");
        toml::from_str::<Profile>(&raw).expect("valid profile")
    }

    #[test]
    fn legacy_table_form_loads() {
        let p = parse(
            r#"
            [custom_headers]
            "X-Foo" = "bar"
        "#,
        );
        assert_eq!(p.custom_headers, vec![("X-Foo".into(), "bar".into())]);
    }

    #[test]
    fn auto_hold_defaults_to_true_when_missing() {
        // Profiles written before the setting existed must default to on.
        assert!(parse("").auto_hold);
    }

    #[test]
    fn legacy_inline_table_form_loads() {
        let p = parse(r#"custom_headers = { "X-Foo" = "bar" }"#);
        assert_eq!(p.custom_headers, vec![("X-Foo".into(), "bar".into())]);
    }

    #[test]
    fn new_array_form_loads_with_duplicates() {
        let p = parse(
            r#"
            custom_headers = [
              ["History-Info", "<sip:1@x.com>;index=1"],
              ["History-Info", "<sip:2@x.com>;index=2"],
            ]
        "#,
        );
        assert_eq!(
            p.custom_headers,
            vec![
                ("History-Info".into(), "<sip:1@x.com>;index=1".into()),
                ("History-Info".into(), "<sip:2@x.com>;index=2".into()),
            ]
        );
    }

    #[test]
    fn missing_field_defaults_to_empty() {
        let p = parse("");
        assert!(p.custom_headers.is_empty());
    }

    #[test]
    fn catchall_defaults_to_true() {
        // Existing profiles on disk have no `catchall` key — they must load as on.
        assert!(parse("").catchall);
        assert!(Profile::default().catchall);
    }

    #[test]
    fn catchall_can_be_disabled() {
        assert!(!parse("catchall = false").catchall);
    }

    #[test]
    fn save_emits_array_form_and_reload_matches() {
        let original = Profile {
            username: "u".into(),
            password: "p".into(),
            domain: "d".into(),
            custom_headers: vec![
                ("History-Info".into(), "<sip:1@x.com>;index=1".into()),
                ("History-Info".into(), "<sip:2@x.com>;index=2".into()),
            ],
            ..Default::default()
        };
        let serialized = toml::to_string_pretty(&original).expect("serialize");
        assert!(
            !serialized.contains("[custom_headers]"),
            "save() must not emit legacy table form, got:\n{serialized}"
        );
        let reloaded: Profile = toml::from_str(&serialized).expect("re-parse");
        assert_eq!(reloaded.custom_headers, original.custom_headers);
    }

    #[test]
    fn resolve_password_uses_inline_by_default() {
        // BASE sets password = "p".
        assert_eq!(parse("").resolve_password().unwrap(), "p");
    }

    #[test]
    fn resolve_password_file_overrides_inline_and_strips_newline() {
        let path = std::env::temp_dir().join(format!("ringo-pw-file-{}", std::process::id()));
        fs::write(&path, "filesecret\n").unwrap();
        let p = parse(&format!("password_file = '{}'", path.display()));
        assert_eq!(p.resolve_password().unwrap(), "filesecret");
        fs::remove_file(&path).ok();
    }

    #[test]
    fn resolve_password_cmd_takes_precedence_over_file_and_inline() {
        let p = parse("password_cmd = \"printf cmdsecret\"\npassword_file = '/does/not/exist'");
        assert_eq!(p.resolve_password().unwrap(), "cmdsecret");
    }

    #[test]
    fn resolve_password_cmd_failure_is_an_error() {
        assert!(
            parse("password_cmd = \"exit 3\"")
                .resolve_password()
                .is_err()
        );
    }

    #[test]
    fn resolve_password_missing_file_is_an_error() {
        assert!(
            parse("password_file = '/does/not/exist'")
                .resolve_password()
                .is_err()
        );
    }
}
