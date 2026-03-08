use ratatui::style::Color;
use serde::Deserialize;
use std::fs;

/// Global ringo configuration, loaded from ~/.config/ringo/ringo.toml.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct RingoConfig {
    pub picker: PickerConfig,
    pub theme: Theme,
    pub baresip: BaresipConfig,
    #[serde(default)]
    pub hooks: Vec<Hook>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Hook {
    pub event: String,
    pub command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    ProfileLoaded,
}

impl HookEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProfileLoaded => "profile_loaded",
        }
    }
}

/// Overrides for auto-detected baresip config values.
///
/// Example in ringo.toml:
/// ```toml
/// [baresip]
/// module_path  = "/usr/lib/baresip/modules"
/// audio_driver = "pulse"
/// sip_cafile   = "/etc/ssl/certs/ca-certificates.crt"
/// sip_capath   = "/etc/ssl/certs"
/// ```
/// Any key that is absent falls back to auto-detection.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct BaresipConfig {
    pub module_path: Option<String>,
    pub audio_driver: Option<String>,
    pub audio_player_device: Option<String>,
    pub audio_source_device: Option<String>,
    pub audio_alert_device: Option<String>,
    pub sip_cafile: Option<String>,
    /// Set to empty string `""` to explicitly disable sip_capath.
    pub sip_capath: Option<String>,
    /// Arbitrary extra baresip config lines appended at the end.
    /// Last value wins, so these override anything in the generated config.
    pub extra: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct PickerConfig {
    /// Profile fields shown as subtitle next to each entry.
    /// Available: aor, username, domain, display_name, transport,
    ///            auth_user, outbound, stun_server, media_enc
    pub info: Vec<String>,
}

impl Default for PickerConfig {
    fn default() -> Self {
        PickerConfig {
            info: vec!["aor".into()],
        }
    }
}

// ─── Theme ───────────────────────────────────────────────────────────────────

/// A color value that can be deserialized from a string like `"cyan"`,
/// `"dark_gray"`, or `"#1a2b3c"`.
#[derive(Clone, Debug)]
pub struct ThemeColor(pub Color);

impl ThemeColor {
    pub fn get(&self) -> Color {
        self.0
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        parse_color(&s)
            .map(ThemeColor)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown color: '{}'", s)))
    }
}

fn parse_color(s: &str) -> Option<Color> {
    match s.to_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" => Some(Color::Gray),
        "dark_gray" | "darkgray" => Some(Color::DarkGray),
        "light_red" | "lightred" => Some(Color::LightRed),
        "light_green" | "lightgreen" => Some(Color::LightGreen),
        "light_yellow" | "lightyellow" => Some(Color::LightYellow),
        "light_blue" | "lightblue" => Some(Color::LightBlue),
        "light_magenta" | "lightmagenta" => Some(Color::LightMagenta),
        "light_cyan" | "lightcyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        s if s.starts_with('#') && s.len() == 7 => {
            let r = u8::from_str_radix(&s[1..3], 16).ok()?;
            let g = u8::from_str_radix(&s[3..5], 16).ok()?;
            let b = u8::from_str_radix(&s[5..7], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

/// Color roles used throughout the UI.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Theme {
    /// Primary accent — logo, picker selection, DTMF input, history search popup.
    pub accent: ThemeColor,
    /// Subdued text — hints, log entries, subtitles, unfocused labels.
    pub subtle: ThemeColor,
    /// Positive states — registered, established call, toggle on, incoming arrow.
    pub success: ThemeColor,
    /// Errors / destructive — muted, missed calls, registration failed, delete.
    pub danger: ThemeColor,
    /// Attention / active — selected call, ringing, MWI, focused form field, registering.
    pub attention: ThemeColor,
    /// Transfer mode — blind and attended transfer input.
    pub transfer: ThemeColor,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: ThemeColor(Color::Cyan),
            subtle: ThemeColor(Color::DarkGray),
            success: ThemeColor(Color::Green),
            danger: ThemeColor(Color::Red),
            attention: ThemeColor(Color::Yellow),
            transfer: ThemeColor(Color::Magenta),
        }
    }
}

pub fn load() -> RingoConfig {
    let path = match config_path() {
        Some(p) => p,
        None => return RingoConfig::default(),
    };
    if !path.exists() {
        return RingoConfig::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn config_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join(".config")
            .join("ringo")
            .join("ringo.toml"),
    )
}
