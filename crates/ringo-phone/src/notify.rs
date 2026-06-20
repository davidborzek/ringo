use std::process::Command;

/// Send a desktop notification. Uses `notify-send` on Linux, `osascript` on macOS.
/// Errors are silently logged via rlog.
pub fn send(summary: &str, body: &str) {
    let result = if cfg!(target_os = "macos") {
        send_macos(summary, body)
    } else {
        send_linux(summary, body)
    };
    if let Err(e) = result {
        crate::rlog!(Debug, "desktop notification failed: {}", e);
    }
}

fn send_linux(summary: &str, body: &str) -> std::io::Result<()> {
    Command::new("notify-send")
        .args(["-a", "ringo", "-i", "call-start", summary, body])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

fn send_macos(summary: &str, body: &str) -> std::io::Result<()> {
    Command::new("osascript")
        .args([
            "-e",
            &format!(
                "display notification \"{}\" with title \"ringo\" subtitle \"{}\"",
                body.replace('"', "\\\""),
                summary.replace('"', "\\\""),
            ),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}
