//! Dotenv parsing shared by the scripting frontends. `--env-file` files and a
//! per-scenario sibling `<stem>.env` are layered into the map that `env(...)`
//! reads (frontend env map → process environment).

use anyhow::{Context as _, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Load and merge several `--env-file`s into one map (later files win).
pub(crate) fn load_env_files(paths: &[PathBuf]) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    for p in paths {
        merge_dotenv(p, &mut env)?;
    }
    Ok(env)
}

/// Parse a dotenv file (`KEY=VALUE` per line; `#` comments and blank lines
/// ignored; optional `export ` prefix; optional surrounding quotes) and merge it
/// into `env`, overwriting existing keys.
pub(crate) fn merge_dotenv(path: &Path, env: &mut HashMap<String, String>) -> Result<()> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read env file {}", path.display()))?;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, value) = line
            .split_once('=')
            .with_context(|| format!("{}:{}: expected KEY=VALUE", path.display(), i + 1))?;
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        env.insert(key.trim().to_string(), value.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::merge_dotenv;

    #[test]
    fn dotenv_parses_comments_quotes_export_and_overrides() {
        let path = std::env::temp_dir().join("ringo_flow_dotenv_test.env");
        std::fs::write(
            &path,
            "# a comment\n\
             \n\
             RF_USER=alice\n\
             export RF_PASS=\"s e cret\"\n\
             RF_DOM='example.com'\n\
             RF_USER=bob\n", // later line overrides
        )
        .unwrap();
        let mut env = std::collections::HashMap::new();
        env.insert("KEEP".into(), "yes".into());
        merge_dotenv(&path, &mut env).unwrap();
        assert_eq!(env["RF_USER"], "bob"); // last wins
        assert_eq!(env["RF_PASS"], "s e cret"); // export + double quotes stripped
        assert_eq!(env["RF_DOM"], "example.com"); // single quotes stripped
        assert_eq!(env["KEEP"], "yes"); // pre-existing keys kept
    }
}
