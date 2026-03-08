use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;

use crate::config::{Hook, HookEvent};
use crate::profile::Profile;

const LOG_PATH: &str = "/tmp/ringo-hooks.log";

/// Run all hooks matching the given event.
/// Commands are spawned in the background and do not block the caller.
/// Errors are logged to `/tmp/ringo-hooks.log`.
pub fn run(hooks: &[Hook], event: HookEvent, profile_name: &str, profile: &Profile) {
    let event_str = event.as_str();
    let profile_json = build_profile_json(profile_name, profile);

    let matching: Vec<_> = hooks.iter().filter(|h| h.event == event_str).collect();
    log(&format!(
        "[{}] hooks::run event={} profile={} total_hooks={} matching={}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        event_str,
        profile_name,
        hooks.len(),
        matching.len(),
    ));

    for hook in matching {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&hook.command)
            .env("RINGO_EVENT", event_str)
            .env("RINGO_PROFILE", profile_name)
            .env("RINGO_PROFILE_JSON", &profile_json)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        match cmd.spawn() {
            Ok(child) => {
                let command = hook.command.clone();
                let event = event_str.to_string();
                std::thread::spawn(move || {
                    if let Ok(output) = child.wait_with_output() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        log(&format!(
                            "[{}] hook {} (event={}, exit={}): {}\n  stdout: {}\n  stderr: {}",
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                            if output.status.success() {
                                "ok"
                            } else {
                                "FAILED"
                            },
                            event,
                            output.status,
                            command,
                            stdout.trim(),
                            stderr.trim(),
                        ));
                    }
                });
            }
            Err(e) => {
                log(&format!(
                    "[{}] hook spawn failed (event={}): {} — {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    event_str,
                    hook.command,
                    e,
                ));
            }
        }
    }
}

fn log(msg: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = writeln!(f, "{}", msg);
    }
}

fn build_profile_json(name: &str, profile: &Profile) -> String {
    let mut obj = serde_json::to_value(profile).unwrap_or_default();
    if let Some(map) = obj.as_object_mut() {
        map.insert("name".into(), serde_json::json!(name));
        map.insert("aor".into(), serde_json::json!(profile.aor()));
        map.remove("password");
    }
    serde_json::to_string(&obj).unwrap_or_default()
}
