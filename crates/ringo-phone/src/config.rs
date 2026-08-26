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
    pub sounds: SoundsConfig,
    #[serde(default)]
    pub hooks: Vec<Hook>,
    /// What went wrong while loading, in the order it was found. Empty on a
    /// clean load.
    ///
    /// A broken config is never fatal — a phone that refuses to start because a
    /// generated theme file is momentarily absent would be worse than one with
    /// default colors. But falling back silently is worse still: the picker runs
    /// before the log file is opened, so a warning at that point goes nowhere at
    /// all, and the user sees a phone that simply ignores its configuration.
    #[serde(skip)]
    pub problems: Vec<String>,
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

/// Custom alert sounds. Each value is either a path to a WAV file (16-bit PCM
/// or G.711 mu-law, mono or stereo) or `"off"` to silence that alert.
///
/// Example in ringo.toml:
/// ```toml
/// [sounds]
/// ring     = "~/sounds/nokia.wav"   # absolute, or ~/ for $HOME
/// ringback = "old-ringback.wav"     # relative to ~/.config/ringo/sounds/
/// busy     = "off"                  # silent
/// ```
/// A key that is absent falls back to `~/.config/ringo/sounds/<alert>.wav` if
/// that file exists, and otherwise to the tone built into ringo. A file that
/// cannot be read or is not a supported WAV falls back the same way, with a
/// warning in the log — a typo must never leave an incoming call silent.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SoundsConfig {
    /// Incoming call. Loops until the call is answered or gone.
    pub ring: Option<String>,
    /// Outgoing call, remote end is ringing. Loops until the call is up or gone.
    pub ringback: Option<String>,
    /// Outgoing call rejected as busy. Played once.
    pub busy: Option<String>,
    /// Outgoing call failed for another reason. Played once.
    pub error: Option<String>,
    /// New voicemail. Played once.
    pub message: Option<String>,
}

impl SoundsConfig {
    /// The alerts ringo knows, paired with what the config says about them.
    /// The names must match `ringo_core`'s alert keys.
    fn entries(&self) -> [(&'static str, Option<&str>); 5] {
        [
            ("ring", self.ring.as_deref()),
            ("ringback", self.ringback.as_deref()),
            ("busy", self.busy.as_deref()),
            ("error", self.error.as_deref()),
            ("message", self.message.as_deref()),
        ]
    }

    /// The `(alert, value)` pairs the engine takes. Alerts left at their
    /// embedded default are omitted entirely.
    pub fn overrides(&self) -> Vec<(String, String)> {
        self.overrides_in(sounds_dir().as_deref())
    }

    /// [`Self::overrides`] against an explicit sounds directory (the drop-in
    /// location), so this is testable without touching $HOME.
    fn overrides_in(&self, dir: Option<&Path>) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (alert, value) in self.entries() {
            match value {
                // Configured: `off` and `tone:` specs pass through untouched,
                // only a path gets resolved.
                Some(v) if is_muted(v) => out.push((alert.into(), "off".into())),
                Some(v) if is_tone(v) => out.push((alert.into(), v.trim().into())),
                Some(v) => out.push((alert.into(), resolve_sound(v, dir))),
                // Not configured: pick up a drop-in file if the user put one there.
                None => {
                    let drop_in = dir.map(|d| d.join(format!("{alert}.wav")));
                    match drop_in {
                        Some(p) if p.is_file() => out.push((alert.into(), p.display().to_string())),
                        _ => {}
                    }
                }
            }
        }
        out
    }
}

/// Values that describe a tone to generate rather than a file to play. They
/// contain slashes, so treating one as a relative path would quietly turn it
/// into `~/.config/ringo/sounds/tone:425/480,0/480`.
fn is_tone(value: &str) -> bool {
    value.trim_start().starts_with("tone:")
}

/// Values that silence an alert instead of naming a file.
fn is_muted(value: &str) -> bool {
    let v = value.trim();
    v.is_empty() || v.eq_ignore_ascii_case("off") || v.eq_ignore_ascii_case("none")
}

/// Resolve a configured sound path: `~/` expands to $HOME, an absolute path is
/// taken as-is, and a bare name resolves against the drop-in sounds directory
/// so `ring = "nokia.wav"` finds `~/.config/ringo/sounds/nokia.wav`.
fn resolve_sound(spec: &str, dir: Option<&Path>) -> String {
    let spec = spec.trim();
    if spec.starts_with("~/") {
        return expand_home(spec).display().to_string();
    }
    let path = Path::new(spec);
    match dir {
        Some(d) if path.is_relative() => d.join(path).display().to_string(),
        _ => spec.to_string(),
    }
}

/// Where ringo looks for drop-in sound files: `~/.config/ringo/sounds/`.
pub fn sounds_dir() -> Option<PathBuf> {
    config_path().map(|p| {
        p.parent()
            .unwrap_or(Path::new("."))
            .join("sounds")
            .to_path_buf()
    })
}

/// How much of the wordmark the picker shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogoMode {
    /// Full block letters when the terminal has room for them, a single line
    /// when it does not. What most people want, so it is the default.
    #[default]
    Auto,
    /// Always the block letters, even where they crowd out the profile list.
    Full,
    /// Always the one-line wordmark.
    Small,
    /// No wordmark at all — the picker starts with the search box.
    Off,
}

impl<'de> Deserialize<'de> for LogoMode {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.to_lowercase().as_str() {
            "auto" => Ok(LogoMode::Auto),
            "full" | "on" | "true" => Ok(LogoMode::Full),
            "small" | "compact" => Ok(LogoMode::Small),
            "off" | "none" | "false" | "hidden" => Ok(LogoMode::Off),
            other => Err(serde::de::Error::custom(format!(
                "unknown logo mode '{other}' — use auto, full, small or off"
            ))),
        }
    }
}

/// The order profiles are listed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PickerOrder {
    /// Most recently started first, then the rest by name. What you reach for
    /// is usually what you reached for last.
    #[default]
    Recent,
    /// Always by name — a list that never moves, for people who navigate by
    /// position rather than by reading.
    Name,
}

impl<'de> Deserialize<'de> for PickerOrder {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.to_lowercase().as_str() {
            "recent" | "recently_used" => Ok(PickerOrder::Recent),
            "name" | "alphabetical" => Ok(PickerOrder::Name),
            other => Err(serde::de::Error::custom(format!(
                "unknown picker order '{other}' — use recent or name"
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct PickerConfig {
    /// Profile fields shown as subtitle next to each entry.
    /// Available: aor, username, domain, display_name, transport,
    ///            auth_user, outbound, stun_server, media_enc, notes,
    ///            and `metadata.<key>` for anything under the profile's
    ///            `[metadata]` table.
    pub info: Vec<String>,
    /// `auto` (default), `full`, `small` or `off`.
    pub logo: LogoMode,
    /// `recent` (default) or `name`.
    pub order: PickerOrder,
}

impl Default for PickerConfig {
    fn default() -> Self {
        PickerConfig {
            info: vec!["aor".into()],
            logo: LogoMode::Auto,
            order: PickerOrder::Recent,
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

/// Record a problem: into the log for the session file, and onto the list the
/// UI shows. Both, because neither alone reaches every case — the log is not
/// open yet during the picker, and the list is gone by the time you read a log.
fn note(problems: &mut Vec<String>, msg: String) {
    crate::rlog!(Warn, "config: {}", msg);
    problems.push(format!("config ignored — {msg}"));
}

/// Loads `path` plus everything it includes. A missing or broken file is a
/// warning, never fatal: a phone that refuses to start because a generated
/// theme file is momentarily absent would be worse than one with default
/// colors.
fn load_from(path: &Path) -> RingoConfig {
    if !path.exists() {
        return RingoConfig::default();
    }
    let mut problems = Vec::new();
    let mut chain = HashSet::new();
    let table = read_merged(path, &mut chain, 0, &mut problems);
    let mut cfg = match table {
        Some(table) => match table.try_into() {
            Ok(cfg) => cfg,
            Err(e) => {
                note(
                    &mut problems,
                    format!("{}: {e} — using defaults", path.display()),
                );
                RingoConfig::default()
            }
        },
        None => RingoConfig::default(),
    };
    cfg.problems = problems;
    cfg
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
fn read_merged(
    path: &Path,
    chain: &mut HashSet<PathBuf>,
    depth: usize,
    problems: &mut Vec<String>,
) -> Option<toml::Table> {
    if depth > MAX_INCLUDE_DEPTH {
        note(
            problems,
            format!(
                "include nested deeper than {MAX_INCLUDE_DEPTH} levels, ignoring {}",
                path.display()
            ),
        );
        return None;
    }
    // Canonicalize so `a.toml` and `./a.toml` count as the same file.
    let key = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !chain.insert(key.clone()) {
        note(
            problems,
            format!("include cycle at {}, ignoring it", path.display()),
        );
        return None;
    }

    let table = read_one(path, problems).map(|own| {
        let mut merged = toml::Table::new();
        for spec in include_paths(&own, path) {
            if let Some(t) = read_merged(&spec, chain, depth + 1, problems) {
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
fn read_one(path: &Path, problems: &mut Vec<String>) -> Option<toml::Table> {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            note(problems, format!("cannot read {}: {e}", path.display()));
            return None;
        }
    };
    match toml::from_str(&raw) {
        Ok(t) => Some(t),
        Err(e) => {
            note(problems, format!("{}: {e}", path.display()));
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
                .map(|spec| match spec.starts_with("~/") {
                    true => expand_home(spec),
                    false => base.join(spec),
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

/// `~/` expands to $HOME; anything else is taken as written. With no $HOME the
/// spec is left alone, so the failure shows up as a missing file, not as a path
/// silently pointing somewhere else.
fn expand_home(spec: &str) -> PathBuf {
    match (spec.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(spec),
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

    /// A private directory to act as the drop-in sounds dir.
    fn sounds_test_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringo-snd-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn no_sound_config_and_no_drop_ins_means_no_overrides() {
        // Everything stays on the tones embedded in the binary.
        let dir = sounds_test_dir("empty");
        assert!(SoundsConfig::default().overrides_in(Some(&dir)).is_empty());
    }

    #[test]
    fn a_drop_in_file_is_picked_up_without_any_config() {
        let dir = sounds_test_dir("dropin");
        fs::write(dir.join("ring.wav"), b"x").unwrap();
        let out = SoundsConfig::default().overrides_in(Some(&dir));
        assert_eq!(
            out,
            vec![(
                "ring".to_string(),
                dir.join("ring.wav").display().to_string()
            )]
        );
    }

    #[test]
    fn an_explicit_path_wins_over_the_drop_in_file() {
        let dir = sounds_test_dir("explicit");
        fs::write(dir.join("ring.wav"), b"x").unwrap();
        let cfg = SoundsConfig {
            ring: Some("/opt/tones/custom.wav".into()),
            ..Default::default()
        };
        let out = cfg.overrides_in(Some(&dir));
        assert_eq!(
            out,
            vec![("ring".to_string(), "/opt/tones/custom.wav".into())]
        );
    }

    #[test]
    fn a_bare_name_resolves_against_the_sounds_dir() {
        let dir = sounds_test_dir("relative");
        let cfg = SoundsConfig {
            ringback: Some("old.wav".into()),
            ..Default::default()
        };
        let out = cfg.overrides_in(Some(&dir));
        assert_eq!(
            out,
            vec![(
                "ringback".to_string(),
                dir.join("old.wav").display().to_string()
            )]
        );
    }

    #[test]
    fn off_survives_even_when_a_drop_in_file_exists() {
        // Muting is an explicit choice; a file lying in the directory must not
        // quietly undo it.
        let dir = sounds_test_dir("muted");
        fs::write(dir.join("ring.wav"), b"x").unwrap();
        let cfg = SoundsConfig {
            ring: Some("Off".into()),
            ..Default::default()
        };
        assert_eq!(
            cfg.overrides_in(Some(&dir)),
            vec![("ring".to_string(), "off".to_string())]
        );
    }

    #[test]
    fn a_tone_spec_reaches_the_engine_untouched() {
        // It has slashes in it; the path resolver must keep its hands off.
        let dir = sounds_test_dir("tone");
        let cfg = SoundsConfig {
            ringback: Some("tone:440+480/2000,0/4000".into()),
            ..Default::default()
        };
        assert_eq!(
            cfg.overrides_in(Some(&dir)),
            vec![(
                "ringback".to_string(),
                "tone:440+480/2000,0/4000".to_string()
            )]
        );
    }

    #[test]
    fn a_tone_spec_wins_over_a_drop_in_file() {
        let dir = sounds_test_dir("tone-dropin");
        fs::write(dir.join("busy.wav"), b"x").unwrap();
        let cfg = SoundsConfig {
            busy: Some("tone:425/480,0/480".into()),
            ..Default::default()
        };
        assert_eq!(
            cfg.overrides_in(Some(&dir)),
            vec![("busy".to_string(), "tone:425/480,0/480".to_string())]
        );
    }

    #[test]
    fn tilde_expands_to_home() {
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        assert_eq!(
            resolve_sound("~/tones/a.wav", None),
            format!("{home}/tones/a.wav")
        );
    }

    #[test]
    fn the_sounds_section_comes_out_of_toml() {
        let cfg = load_files(&[(
            "ringo.toml",
            "[sounds]\nring = \"/tmp/r.wav\"\nbusy = \"off\"\n",
        )]);
        assert_eq!(cfg.sounds.ring.as_deref(), Some("/tmp/r.wav"));
        assert_eq!(cfg.sounds.busy.as_deref(), Some("off"));
        assert_eq!(cfg.sounds.message, None);
    }

    #[test]
    fn a_broken_config_reports_what_went_wrong() {
        // The picker runs before the log file is opened, so a warning there
        // reaches nobody. The config has to carry the problem itself.
        let cfg = load_files(&[("ringo.toml", "include = \"theme.toml\"\n")]);
        assert_eq!(cfg.problems.len(), 1, "{:?}", cfg.problems);
        assert!(
            cfg.problems[0].contains("sequence"),
            "must say what the file got wrong: {:?}",
            cfg.problems
        );
    }

    #[test]
    fn an_unparseable_file_is_reported() {
        let cfg = load_files(&[("ringo.toml", "this is not = = toml\n")]);
        assert_eq!(cfg.problems.len(), 1);
    }

    #[test]
    fn a_broken_include_is_reported_while_the_rest_loads() {
        let cfg = load_files(&[
            (
                "ringo.toml",
                "include = [\"bad.toml\"]\n[theme]\naccent = \"blue\"\n",
            ),
            ("bad.toml", "this is not = = toml\n"),
        ]);
        assert_eq!(
            cfg.theme.accent.0,
            Color::Blue,
            "the good part still applies"
        );
        assert_eq!(cfg.problems.len(), 1, "and the bad part is named");
        assert!(cfg.problems[0].contains("bad.toml"));
    }

    #[test]
    fn a_clean_config_reports_nothing() {
        let cfg = load_files(&[("ringo.toml", "[theme]\naccent = \"blue\"\n")]);
        assert!(cfg.problems.is_empty(), "{:?}", cfg.problems);
    }

    #[test]
    fn a_missing_include_is_reported_too() {
        // Documented as tolerable — a generated theme file may not exist yet —
        // but the user still gets to know it was skipped.
        let cfg = load_files(&[("ringo.toml", "include = [\"nope.toml\"]\n")]);
        assert_eq!(cfg.problems.len(), 1);
        assert!(cfg.problems[0].contains("nope.toml"));
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
