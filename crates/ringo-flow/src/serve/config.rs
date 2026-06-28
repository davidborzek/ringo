//! The `serve` configuration, parsed from a `monitor.toml`: where to listen,
//! which `ringo-flow` binary to spawn per run, and the monitors to schedule /
//! expose. A *monitor* names a scenario file (which may itself hold a whole
//! suite of scenarios) plus an optional schedule.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Top-level `monitor.toml`.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// HTTP listen address (default `127.0.0.1:9090`).
    #[serde(default = "default_listen")]
    pub listen: String,
    /// The `ringo-flow` binary spawned for each run. Defaults to the running
    /// executable, so a single binary serves and runs.
    #[serde(default)]
    pub binary: Option<PathBuf>,
    /// Default per-run timeout (e.g. `"300s"`), overridable per monitor.
    #[serde(default = "default_timeout")]
    pub timeout: String,
    /// Prometheus `/metrics` endpoint settings (`[metrics]` table).
    #[serde(default)]
    pub metrics: MetricsConfig,
    /// The configured monitors (`[[monitor]]` tables).
    #[serde(rename = "monitor", default)]
    pub monitors: Vec<MonitorConfig>,
}

/// The `[metrics]` table: controls the Prometheus `/metrics` endpoint.
#[derive(Debug, Deserialize)]
pub struct MetricsConfig {
    /// Whether to expose `/metrics` at all (default `true`). When `false` the
    /// route isn't registered and returns 404.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// If set, `/metrics` requires `Authorization: Bearer <token>`. Unset means
    /// no auth (fine when bound to localhost / behind a trusted network).
    #[serde(default)]
    pub bearer_token: Option<String>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bearer_token: None,
        }
    }
}

/// One `[[monitor]]` entry: a named scenario file run on an optional schedule.
#[derive(Debug, Deserialize, Clone)]
pub struct MonitorConfig {
    /// Unique name — the `monitor` metric label and the `/run/<name>` path.
    pub name: String,
    /// Path to the `.rhai` scenario file or a directory of them.
    pub path: PathBuf,
    /// Cron schedule (5- or 6-field). Omit to only run on demand via `/run`.
    #[serde(default)]
    pub schedule: Option<String>,
    /// Per-run timeout override (e.g. `"60s"`); falls back to the global one.
    #[serde(default)]
    pub timeout: Option<String>,
    /// dotenv file(s) passed through as `--env-file`.
    #[serde(default)]
    pub env_file: Vec<PathBuf>,
    /// `--scenario` name filter applied within the file.
    #[serde(default)]
    pub scenario: Option<String>,
    /// `--tag` filters applied within the file.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_listen() -> String {
    "127.0.0.1:9090".to_string()
}
fn default_timeout() -> String {
    "300s".to_string()
}
fn default_true() -> bool {
    true
}

impl Config {
    /// Parse and validate a `monitor.toml`. Relative scenario/env paths are
    /// resolved against the config file's directory (not the server's cwd), so a
    /// config is portable regardless of where the server is launched from.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read config {}", path.display()))?;
        let mut cfg: Config =
            toml::from_str(&text).with_context(|| format!("parse config {}", path.display()))?;
        if let Some(base) = path.parent() {
            for m in &mut cfg.monitors {
                m.path = resolve(base, &m.path);
                for env in &mut m.env_file {
                    *env = resolve(base, env);
                }
            }
        }
        cfg.validate()?;
        Ok(cfg)
    }

    /// The global default per-run timeout, parsed.
    pub fn default_timeout(&self) -> Result<Duration> {
        crate::engine::duration::parse_duration(&self.timeout)
            .map_err(|e| anyhow::anyhow!("invalid timeout `{}`: {e}", self.timeout))
    }

    /// Reject empty/duplicate names, missing paths, bad cron expressions and bad
    /// timeouts up front, so a misconfigured server fails to start rather than at
    /// runtime.
    fn validate(&self) -> Result<()> {
        self.default_timeout()?;
        if self.monitors.is_empty() {
            bail!("no [[monitor]] entries in the config");
        }
        let mut seen = HashSet::new();
        for m in &self.monitors {
            if m.name.trim().is_empty() {
                bail!("a [[monitor]] has an empty name");
            }
            if !seen.insert(&m.name) {
                bail!("duplicate monitor name `{}`", m.name);
            }
            if !m.path.exists() {
                bail!("monitor `{}`: path not found: {}", m.name, m.path.display());
            }
            if let Some(expr) = &m.schedule {
                expr.parse::<croner::Cron>()
                    .with_context(|| format!("invalid schedule for `{}`: `{expr}`", m.name))?;
            }
            if let Some(t) = &m.timeout {
                crate::engine::duration::parse_duration(t)
                    .map_err(|e| anyhow::anyhow!("invalid timeout for `{}`: {e}", m.name))?;
            }
        }
        Ok(())
    }
}

/// Resolve `p` against `base` unless it is already absolute.
fn resolve(base: &Path, p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

impl MonitorConfig {
    /// This monitor's effective timeout: its override, else the server default.
    pub fn timeout(&self, default: Duration) -> Duration {
        self.timeout
            .as_deref()
            .and_then(|t| crate::engine::duration::parse_duration(t).ok())
            .unwrap_or(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique temp dir for one test (no external tempfile dep).
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringo-serve-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_joins_relative_keeps_absolute() {
        let base = Path::new("/etc/ringo");
        assert_eq!(
            resolve(base, Path::new("tests/a.rhai")),
            PathBuf::from("/etc/ringo/tests/a.rhai")
        );
        assert_eq!(
            resolve(base, Path::new("/abs/a.rhai")),
            PathBuf::from("/abs/a.rhai")
        );
    }

    #[test]
    fn load_resolves_relative_path_against_config_dir() {
        let dir = temp_dir("relpath");
        std::fs::write(dir.join("scn.rhai"), "scenario(\"x\", || {});").unwrap();
        std::fs::write(
            dir.join("m.toml"),
            "[[monitor]]\nname = \"x\"\npath = \"scn.rhai\"\n",
        )
        .unwrap();

        let cfg = Config::load(&dir.join("m.toml")).expect("load");
        // The relative `scn.rhai` resolves next to the config, not the cwd.
        assert_eq!(cfg.monitors[0].path, dir.join("scn.rhai"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rejects_missing_scenario_path() {
        let dir = temp_dir("missing");
        std::fs::write(
            dir.join("m.toml"),
            "[[monitor]]\nname = \"x\"\npath = \"nope.rhai\"\n",
        )
        .unwrap();

        let err = Config::load(&dir.join("m.toml")).unwrap_err().to_string();
        assert!(err.contains("path not found"), "{err}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_rejects_duplicate_names() {
        let dir = temp_dir("dup");
        std::fs::write(dir.join("a.rhai"), "").unwrap();
        std::fs::write(
            dir.join("m.toml"),
            "[[monitor]]\nname = \"x\"\npath = \"a.rhai\"\n\
             [[monitor]]\nname = \"x\"\npath = \"a.rhai\"\n",
        )
        .unwrap();

        let err = Config::load(&dir.join("m.toml")).unwrap_err().to_string();
        assert!(err.contains("duplicate"), "{err}");

        std::fs::remove_dir_all(&dir).ok();
    }
}
