use ratatui::style::Color;
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

/// How many levels of `include` are followed before giving up. Deep enough for
/// any real layout, shallow enough that a mistake surfaces as a warning rather
/// than a stack of file reads.
const MAX_INCLUDE_DEPTH: usize = 8;

/// Global ringo configuration, loaded from ~/.config/ringo/ringo.toml.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct RingoConfig {
    /// Other TOML files to fold into this one, applied in order before the
    /// including file's own keys. Relative paths resolve against the directory
    /// of the file naming them, and a leading `~/` expands to $HOME.
    pub include: Vec<String>,
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
    CallIncoming,
    CallOutgoing,
    CallEnded,
}

impl HookEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProfileLoaded => "profile_loaded",
            Self::CallIncoming => "call_incoming",
            Self::CallOutgoing => "call_outgoing",
            Self::CallEnded => "call_ended",
        }
    }
}

/// Overrides for auto-detected baresip config values.
///
/// Example in ringo.toml:
/// ```toml
/// [baresip]
/// audio_driver = "pulse"
/// sip_cafile   = "/etc/ssl/certs/ca-certificates.crt"
/// sip_capath   = "/etc/ssl/certs"
/// ```
/// Any key that is absent falls back to auto-detection.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct BaresipConfig {
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
    match config_path() {
        Some(p) => load_from(&p),
        None => RingoConfig::default(),
    }
}

/// Loads `path` plus everything it includes. A missing or broken file is a
/// warning, never fatal: a phone that refuses to start because a generated
/// theme file is momentarily absent would be worse than one with default
/// colors.
fn load_from(path: &Path) -> RingoConfig {
    if !path.exists() {
        return RingoConfig::default();
    }
    let mut chain = HashSet::new();
    let Some(table) = read_merged(path, &mut chain, 0) else {
        return RingoConfig::default();
    };
    match table.try_into() {
        Ok(cfg) => cfg,
        Err(e) => {
            crate::rlog!(Warn, "config parse error ({}): {}", path.display(), e);
            RingoConfig::default()
        }
    }
}

/// Reads one file and folds its includes underneath it: includes are applied in
/// listed order, then the including file's own keys go on top. So the file you
/// edit by hand always wins over a file it pulled in, and a later include wins
/// over an earlier one.
///
/// `chain` holds the files currently being read — the ancestors of this one, not
/// everything seen so far. That distinction matters: two files legitimately
/// including the same third file is a diamond, not a cycle, and treating it as
/// one would silently drop the second application and break the ordering above.
fn read_merged(path: &Path, chain: &mut HashSet<PathBuf>, depth: usize) -> Option<toml::Table> {
    if depth > MAX_INCLUDE_DEPTH {
        crate::rlog!(
            Warn,
            "config include nested deeper than {} levels, ignoring {}",
            MAX_INCLUDE_DEPTH,
            path.display()
        );
        return None;
    }
    // Canonicalize so `a.toml` and `./a.toml` count as the same file.
    let key = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !chain.insert(key.clone()) {
        crate::rlog!(
            Warn,
            "config include cycle at {}, ignoring it",
            path.display()
        );
        return None;
    }

    let table = read_one(path).map(|own| {
        let mut merged = toml::Table::new();
        for spec in include_paths(&own, path) {
            if let Some(t) = read_merged(&spec, chain, depth + 1) {
                merge(&mut merged, t);
            }
        }
        merge(&mut merged, own);
        merged
    });

    chain.remove(&key);
    table
}

/// Reads and parses one file, reporting either failure as a warning.
fn read_one(path: &Path) -> Option<toml::Table> {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            crate::rlog!(Warn, "config read error ({}): {}", path.display(), e);
            return None;
        }
    };
    match toml::from_str(&raw) {
        Ok(t) => Some(t),
        Err(e) => {
            crate::rlog!(Warn, "config parse error ({}): {}", path.display(), e);
            None
        }
    }
}

/// The `include` entries of `table`, resolved against the directory holding the
/// file that named them. `~/` expands to $HOME; absolute paths are taken as-is.
fn include_paths(table: &toml::Table, path: &Path) -> Vec<PathBuf> {
    let base = path.parent().unwrap_or(Path::new("."));
    table
        .get("include")
        .and_then(toml::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(toml::Value::as_str)
                .map(|spec| match spec.strip_prefix("~/") {
                    Some(rest) => match std::env::var("HOME") {
                        Ok(home) => PathBuf::from(home).join(rest),
                        Err(_) => PathBuf::from(spec),
                    },
                    None => base.join(spec),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Recursive table merge: `over` wins, but only for the keys it actually sets,
/// so an included `[theme]` and a local `[theme]` combine per color instead of
/// one replacing the other wholesale. Non-table values (including arrays) are
/// replaced outright — there is no sensible way to merge two `hooks` lists that
/// does not surprise someone.
fn merge(base: &mut toml::Table, over: toml::Table) {
    for (k, v) in over {
        match (base.get_mut(&k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    /// Writes the named files into a private directory and loads the first one.
    fn load_files(files: &[(&str, &str)]) -> RingoConfig {
        let dir = std::env::temp_dir().join(format!(
            "ringo-cfg-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        for (name, body) in files {
            let path = dir.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, body).unwrap();
        }
        load_from(&dir.join(files[0].0))
    }

    #[test]
    fn include_supplies_values_the_main_file_omits() {
        let cfg = load_files(&[
            ("ringo.toml", "include = [\"theme.toml\"]\n"),
            ("theme.toml", "[theme]\naccent = \"#ff0000\"\n"),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Rgb(0xff, 0, 0));
    }

    #[test]
    fn main_file_wins_over_its_includes() {
        let cfg = load_files(&[
            (
                "ringo.toml",
                "include = [\"theme.toml\"]\n[theme]\naccent = \"green\"\n",
            ),
            ("theme.toml", "[theme]\naccent = \"#ff0000\"\n"),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Green);
    }

    #[test]
    fn tables_merge_per_key_rather_than_wholesale() {
        // The include sets accent, the main file sets subtle: both must survive,
        // and the untouched roles must keep their defaults.
        let cfg = load_files(&[
            (
                "ringo.toml",
                "include = [\"theme.toml\"]\n[theme]\nsubtle = \"blue\"\n",
            ),
            ("theme.toml", "[theme]\naccent = \"#ff0000\"\n"),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Rgb(0xff, 0, 0));
        assert_eq!(cfg.theme.subtle.0, Color::Blue);
        assert_eq!(cfg.theme.success.0, Theme::default().success.0);
    }

    #[test]
    fn later_include_wins_over_earlier() {
        let cfg = load_files(&[
            ("ringo.toml", "include = [\"a.toml\", \"b.toml\"]\n"),
            ("a.toml", "[theme]\naccent = \"red\"\n"),
            ("b.toml", "[theme]\naccent = \"blue\"\n"),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Blue);
    }

    #[test]
    fn include_paths_resolve_against_the_including_file() {
        let cfg = load_files(&[
            ("ringo.toml", "include = [\"themes/pick.toml\"]\n"),
            ("themes/pick.toml", "[theme]\naccent = \"magenta\"\n"),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Magenta);
    }

    #[test]
    fn includes_nest() {
        let cfg = load_files(&[
            ("ringo.toml", "include = [\"mid.toml\"]\n"),
            ("mid.toml", "include = [\"leaf.toml\"]\n"),
            ("leaf.toml", "[theme]\naccent = \"yellow\"\n"),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Yellow);
    }

    #[test]
    fn a_missing_include_leaves_the_rest_intact() {
        // A generated theme file may not exist yet; that must not cost the user
        // the settings they wrote by hand.
        let cfg = load_files(&[(
            "ringo.toml",
            "include = [\"nope.toml\"]\n[theme]\naccent = \"blue\"\n",
        )]);
        assert_eq!(cfg.theme.accent.0, Color::Blue);
    }

    #[test]
    fn a_broken_include_leaves_the_rest_intact() {
        let cfg = load_files(&[
            (
                "ringo.toml",
                "include = [\"bad.toml\"]\n[theme]\naccent = \"blue\"\n",
            ),
            ("bad.toml", "this is not = = toml\n"),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Blue);
    }

    #[test]
    fn an_include_cycle_terminates() {
        let cfg = load_files(&[
            (
                "ringo.toml",
                "include = [\"loop.toml\"]\n[theme]\nsubtle = \"blue\"\n",
            ),
            (
                "loop.toml",
                "include = [\"ringo.toml\"]\n[theme]\naccent = \"red\"\n",
            ),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Red);
        assert_eq!(cfg.theme.subtle.0, Color::Blue);
    }

    #[test]
    fn other_sections_come_through_an_include() {
        let cfg = load_files(&[
            ("ringo.toml", "include = [\"extra.toml\"]\n"),
            (
                "extra.toml",
                "[baresip]\naudio_driver = \"pulse\"\n\n[[hooks]]\nevent = \"call_incoming\"\ncommand = \"true\"\n",
            ),
        ]);
        assert_eq!(cfg.baresip.audio_driver.as_deref(), Some("pulse"));
        assert_eq!(cfg.hooks.len(), 1);
    }

    #[test]
    fn a_file_cannot_include_itself() {
        let cfg = load_files(&[(
            "ringo.toml",
            "include = [\"ringo.toml\"]\n[theme]\naccent = \"blue\"\n",
        )]);
        assert_eq!(cfg.theme.accent.0, Color::Blue);
    }

    #[test]
    fn two_files_may_include_the_same_third_file() {
        // Two different files include the same third file. This is NOT a cycle.
        let cfg = load_files(&[
            ("ringo.toml", "include = [\"a.toml\", \"b.toml\"]\n"),
            ("a.toml", "include = [\"common.toml\"]\n"),
            (
                "b.toml",
                "include = [\"common.toml\"]\n[theme]\nsubtle = \"blue\"\n",
            ),
            ("common.toml", "[theme]\naccent = \"red\"\n"),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Red, "common.toml must apply");
        assert_eq!(cfg.theme.subtle.0, Color::Blue);
    }

    #[test]
    fn a_shared_include_still_follows_the_documented_order() {
        // a.toml overrides its own include; b.toml comes later and pulls the same
        // shared file in, so by the documented order b's value must win.
        let cfg = load_files(&[
            ("ringo.toml", "include = [\"a.toml\", \"b.toml\"]\n"),
            (
                "a.toml",
                "include = [\"common.toml\"]\n[theme]\naccent = \"green\"\n",
            ),
            ("b.toml", "include = [\"common.toml\"]\n"),
            ("common.toml", "[theme]\naccent = \"red\"\n"),
        ]);
        assert_eq!(cfg.theme.accent.0, Color::Red);
    }
}
