//! ringo's per-user SIP profiles: the CLI listing and the recently-used order.
//!
//! The `profile.toml` format itself lives in [`ringo_profile::profile`]; this
//! module re-exports it so existing `ringo::profile::…` paths keep working.

// Re-export the whole format surface so `ringo::profile::…` paths keep working.
// The bin target re-declares this module privately, where some re-exports have
// no caller — the lib target needs them all as public API.
#[allow(unused_imports)]
pub use ringo_profile::profile::{
    Profile, dirs_base, list_names, load, profile_dir, profiles_dir, rename, save,
};

use anyhow::{Context, Result};
use std::{fs, path::PathBuf};

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
