use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf, process::Command};

const CONFIG_TEMPLATE: &str = include_str!("../assets/config.tera");

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
    #[serde(default = "default_true")]
    pub notify: bool,
    #[serde(default = "default_true")]
    pub mwi: bool,
}

fn default_true() -> bool {
    true
}

impl Profile {
    /// Generate the single active line for the baresip `accounts` file.
    pub fn to_accounts_line(&self) -> String {
        let display = self
            .display_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| format!("{} ", s))
            .unwrap_or_default();

        let transport = self
            .transport
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| format!(";transport={}", s))
            .unwrap_or_default();

        let auth_user = self
            .auth_user
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.username);

        let mut line = format!(
            "{}<sip:{}@{}{}>; auth_user={};auth_pass={}",
            display, self.username, self.domain, transport, auth_user, self.password
        );

        if let Some(v) = self.outbound.as_deref().filter(|s| !s.is_empty()) {
            line.push_str(&format!(";outbound={}", v));
        }
        if let Some(v) = self.stun_server.as_deref().filter(|s| !s.is_empty()) {
            line.push_str(&format!(";stunserver={}", v));
        }
        if let Some(v) = self.media_enc.as_deref().filter(|s| !s.is_empty()) {
            line.push_str(&format!(";mediaenc={}", v));
        }

        line
    }

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

pub fn list(plain: bool) -> Result<()> {
    let names = list_names()?;
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

// ─── Load / Save ─────────────────────────────────────────────────────────────

pub fn load(name: &str) -> Result<Profile> {
    let path = profile_dir(name)?.join("profile.toml");
    let raw =
        fs::read_to_string(&path).with_context(|| format!("Cannot read profile '{}'", name))?;
    toml::from_str(&raw).with_context(|| format!("Invalid profile.toml for '{}'", name))
}

pub fn save(name: &str, profile: &Profile) -> Result<()> {
    let dir = profile_dir(name)?;
    fs::create_dir_all(&dir)?;

    let toml_str = toml::to_string_pretty(profile)?;
    fs::write(dir.join("profile.toml"), toml_str)?;

    Ok(())
}

// ─── Config generation ────────────────────────────────────────────────────────

pub fn generate_config_content(profile: &Profile, port: u16) -> Result<String> {
    let overrides = &crate::config::load().baresip;

    let module_path = overrides
        .module_path
        .clone()
        .unwrap_or_else(detect_module_path);
    let audio_driver = overrides
        .audio_driver
        .as_deref()
        .unwrap_or_else(|| detect_audio_driver(&module_path));
    let audio_player_device = overrides
        .audio_player_device
        .as_deref()
        .unwrap_or("default");
    let audio_source_device = overrides
        .audio_source_device
        .as_deref()
        .unwrap_or("default");
    let audio_alert_device = overrides.audio_alert_device.as_deref().unwrap_or("default");
    let sip_cafile = overrides
        .sip_cafile
        .clone()
        .unwrap_or_else(detect_sip_cafile);
    let sip_capath: Option<String> = match &overrides.sip_capath {
        Some(s) if s.is_empty() => None, // explicit disable via ""
        Some(s) => Some(s.clone()),
        None => detect_sip_capath(),
    };

    let mut ctx = tera::Context::new();
    ctx.insert("module_path", &module_path);
    ctx.insert("audio_driver", &audio_driver);
    ctx.insert("audio_player_device", &audio_player_device);
    ctx.insert("audio_source_device", &audio_source_device);
    ctx.insert("audio_alert_device", &audio_alert_device);
    ctx.insert("port", &port);
    ctx.insert("sip_cafile", &sip_cafile);
    ctx.insert("sip_capath", &sip_capath);
    ctx.insert("mwi", &profile.mwi);

    tera::Tera::one_off(CONFIG_TEMPLATE, &ctx, false)
        .context("Failed to render baresip config template")
}

fn detect_module_path() -> String {
    // Try pkg-config first
    if let Ok(out) = Command::new("pkg-config")
        .args(["--variable=moduledir", "baresip"])
        .output()
    {
        if out.status.success() {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() && std::path::Path::new(&path).exists() {
                return path;
            }
        }
    }

    // Known paths ordered by likelihood
    let candidates = [
        "/opt/homebrew/lib/baresip/modules", // macOS ARM (Homebrew)
        "/usr/local/lib/baresip/modules",    // macOS Intel (Homebrew)
        "/usr/lib/x86_64-linux-gnu/baresip/modules", // Debian/Ubuntu x86_64
        "/usr/lib/aarch64-linux-gnu/baresip/modules", // Debian/Ubuntu ARM64
        "/usr/lib/baresip/modules",          // Arch Linux / generic
        "/usr/lib64/baresip/modules",        // Fedora/RHEL
    ];

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }

    "/usr/lib/baresip/modules".to_string()
}

fn detect_audio_driver(module_path: &str) -> &'static str {
    #[cfg(target_os = "macos")]
    return "coreaudio";

    #[cfg(not(target_os = "macos"))]
    {
        let base = std::path::Path::new(module_path);
        for driver in &["pipewire", "pulse", "alsa"] {
            if base.join(format!("{}.so", driver)).exists() {
                return driver;
            }
        }
        "alsa"
    }
}

fn detect_sip_cafile() -> String {
    let candidates = [
        "/etc/ssl/cert.pem",                  // macOS
        "/etc/ssl/certs/ca-certificates.crt", // Debian/Ubuntu/Arch
        "/etc/pki/tls/certs/ca-bundle.crt",   // Fedora/RHEL
    ];

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }

    "/etc/ssl/certs/ca-certificates.crt".to_string()
}

fn detect_sip_capath() -> Option<String> {
    #[cfg(target_os = "macos")]
    return None;

    #[cfg(not(target_os = "macos"))]
    {
        let path = "/etc/ssl/certs";
        if std::path::Path::new(path).exists() {
            Some(path.to_string())
        } else {
            None
        }
    }
}

// ─── Interactive profile picker (ratatui) ────────────────────────────────────

/// Open the interactive profile picker; loops until a profile is selected to start.
/// Manages the terminal lifecycle; stays in alternate screen on success so the
/// TUI can take over seamlessly.
pub fn pick_profile() -> Result<String> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = pick_profile_loop(&mut terminal);

    if result.is_err() {
        let _ = disable_raw_mode();
        let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    }

    result
}

fn pick_profile_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<String> {
    use crate::picker::{PickerAction, PickerItem};
    let config = crate::config::load();
    let theme = &config.theme;
    loop {
        let names = list_names().unwrap_or_default();
        let items: Vec<PickerItem> = names
            .iter()
            .map(|name| {
                let subtitle = load(name)
                    .map(|p| build_subtitle(&p, &config.picker.info))
                    .unwrap_or_default();
                PickerItem {
                    name: name.clone(),
                    subtitle,
                }
            })
            .collect();
        match crate::picker::run(terminal, &items, theme)? {
            PickerAction::Start(name) => return Ok(name),
            PickerAction::New => {
                if let Some((name, profile)) =
                    crate::form::run_form(terminal, None, &Profile::default(), &names, theme)?
                {
                    save(&name, &profile)?;
                }
            }
            PickerAction::Edit(name) => {
                let current = load(&name)?;
                if let Some((_, profile)) =
                    crate::form::run_form(terminal, Some(&name), &current, &[], theme)?
                {
                    save(&name, &profile)?;
                }
            }
            PickerAction::Delete(name) => {
                if crate::form::run_confirm(terminal, &name, theme)? {
                    fs::remove_dir_all(profile_dir(&name)?)?;
                }
            }
        }
    }
}

fn build_subtitle(profile: &Profile, fields: &[String]) -> String {
    fields
        .iter()
        .filter_map(|f| match f.as_str() {
            "aor" => Some(profile.aor()),
            "username" => Some(profile.username.clone()),
            "domain" => Some(profile.domain.clone()),
            "display_name" => profile.display_name.clone().filter(|s| !s.is_empty()),
            "transport" => Some(
                profile
                    .transport
                    .as_deref()
                    .unwrap_or("default")
                    .to_string(),
            ),
            "auth_user" => profile.auth_user.clone().filter(|s| !s.is_empty()),
            "outbound" => profile.outbound.clone().filter(|s| !s.is_empty()),
            "stun_server" => profile.stun_server.clone().filter(|s| !s.is_empty()),
            "media_enc" => Some(profile.media_enc.as_deref().unwrap_or("none").to_string()),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("  ·  ")
}
