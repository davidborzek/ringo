use std::process::Command;

use crate::config::{Hook, HookEvent};
use crate::profile::Profile;
use crate::rlog;

/// Run all hooks matching the given event.
/// Commands are spawned in the background and do not block the caller.
pub fn run(hooks: &[Hook], event: HookEvent, profile_name: &str, profile: &Profile) {
    let event_str = event.as_str();
    let profile_json = build_profile_json(profile_name, profile);

    let matching: Vec<_> = hooks.iter().filter(|h| h.event == event_str).collect();
    rlog!(
        Info,
        "hooks::run event={} profile={} total_hooks={} matching={}",
        event_str,
        profile_name,
        hooks.len(),
        matching.len(),
    );

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
                        rlog!(
                            Info,
                            "hook {} (event={}, exit={}): {}\n  stdout: {}\n  stderr: {}",
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
                        );
                    }
                });
            }
            Err(e) => {
                rlog!(
                    Error,
                    "hook spawn failed (event={}): {} — {}",
                    event_str,
                    hook.command,
                    e,
                );
            }
        }
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
