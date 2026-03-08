use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Profile {
    pub display_name: Option<String>,
    pub username: String,
    pub auth_user: Option<String>,
    pub password: String,
    pub domain: String,
    pub transport: Option<String>,
    pub outbound: Option<String>,
    pub stun_server: Option<String>,
    pub media_enc: Option<String>,
    pub regint: Option<u32>,
    #[serde(default)]
    pub custom_headers: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub notify: bool,
    #[serde(default = "default_true")]
    pub mwi: bool,
}

fn default_true() -> bool {
    true
}

impl Profile {
    /// The SIP AOR string for this profile.
    pub fn aor(&self) -> String {
        format!("sip:{}@{}", self.username, self.domain)
    }
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

pub fn profile_dir(name: &str) -> Result<PathBuf> {
    Ok(profiles_dir()?.join(name))
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
