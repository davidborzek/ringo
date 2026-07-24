use crossterm::event::{KeyCode, KeyModifiers};

use super::app::App;

pub const COMMANDS: &[&str] = &[
    "accept", "contacts", "dial", "dtmf", "edit", "hangup", "help", "history", "hold", "log",
    "mute", "quit", "resume", "switch", "transfer", "xfer",
];

impl App {
    pub fn execute_command(&mut self) {
        let raw = self.command.input.trim().to_string();
        self.command.input.clear();
        self.command.active = false;

        if raw.is_empty() {
            return;
        }

        let mut parts = raw.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("").trim();

        match cmd {
            "q" | "quit" => {
                self.phone.hangup_all();
                self.quit = true;
            }
            "log" | "l" => {
                self.log.show = !self.log.show;
                if self.log.show {
                    self.close_overlays();
                    self.log.show = true;
                    self.refresh_log();
                }
                self.log.scroll = 0;
            }
            "history" | "c" => {
                let open = !self.call_history.show;
                self.close_overlays();
                self.call_history.show = open;
                if open {
                    self.refresh_call_history();
                }
                self.log.scroll = 0;
            }
            "contacts" | "f" => {
                let open = !self.contacts_state.show;
                self.close_overlays();
                self.contacts_state.show = open;
                if open {
                    self.contacts_state.selected = 0;
                    self.contacts_state.search_query.clear();
                    self.contacts_state.search_mode = false;
                }
            }
            "help" | "?" => {
                let open = !self.help_show;
                self.close_overlays();
                self.help_show = open;
            }
            "edit" => {
                if !self.has_any_call() {
                    self.edit_profile = true;
                    self.quit = true;
                } else {
                    self.command.error = Some("Cannot edit during call".into());
                }
            }
            "switch" => {
                self.phone.hangup_all();
                self.switch_to = true;
                self.quit = true;
            }
            // Call-control commands are shared with remote control via `dispatch`.
            _ => {
                if let Err(e) = self.dispatch(cmd, arg) {
                    self.command.error = Some(e);
                }
            }
        }
    }

    /// Execute a call-control command. Shared by the command line and by remote
    /// control (`ringo control …`). Returns a human-readable success message,
    /// or an error string for the caller to surface.
    pub fn dispatch(&mut self, cmd: &str, arg: &str) -> Result<String, String> {
        match cmd {
            "d" | "dial" => {
                if arg.is_empty() {
                    return Err("Usage: dial <number>".into());
                }
                // `App::dial` re-renders dynamic custom headers (e.g. `$uuid`).
                self.dial(arg);
                crate::history::push(&mut self.dial.history, arg.to_string());
                Ok(format!("Dialing {arg}"))
            }
            "hangup" => {
                if self.has_any_call() {
                    self.phone.hangup();
                    Ok("Hung up".into())
                } else {
                    Err("No active call".into())
                }
            }
            "a" | "accept" => {
                if self.has_incoming_ringing() {
                    self.accept_incoming();
                    Ok("Accepted".into())
                } else {
                    Err("No incoming call".into())
                }
            }
            "hold" => {
                if self.in_active_call() {
                    self.phone.hold();
                    let idx = self.selected_call;
                    if let Some(c) = self.calls.get_mut(idx) {
                        c.state = super::app::CallState::OnHold;
                    }
                    Ok("On hold".into())
                } else {
                    Err("No active call".into())
                }
            }
            "resume" => {
                if self.selected_call_on_hold() {
                    self.phone.resume();
                    let idx = self.selected_call;
                    if let Some(c) = self.calls.get_mut(idx) {
                        c.state = super::app::CallState::Established;
                    }
                    Ok("Resumed".into())
                } else {
                    Err("No call on hold".into())
                }
            }
            "mute" => {
                if self.in_active_call() {
                    self.muted = !self.muted;
                    self.phone.mute();
                    Ok(if self.muted { "Muted" } else { "Unmuted" }.into())
                } else {
                    Err("No active call".into())
                }
            }
            "xfer" | "transfer" => {
                if arg.is_empty() {
                    Err("Usage: transfer <uri>".into())
                } else if self.in_active_call() {
                    let aor = self.account_aor.clone();
                    let uri = normalize_sip_uri(arg, &aor);
                    self.phone.transfer(&uri);
                    Ok(format!("Transferring to {uri}"))
                } else {
                    Err("No active call".into())
                }
            }
            "dtmf" => {
                if arg.is_empty() {
                    return Err("Usage: dtmf <digits>".into());
                }
                if !self.in_active_call() {
                    return Err("No active call".into());
                }
                // Whitespace is allowed for readability (e.g. "1 2 3#").
                let digits: Vec<char> = arg.chars().filter(|c| !c.is_whitespace()).collect();
                if let Some(bad) = digits.iter().find(|c| !is_dtmf_digit(**c)) {
                    return Err(format!("Invalid DTMF digit: {bad}"));
                }
                for c in &digits {
                    self.send_dtmf(*c);
                }
                Ok(format!("Sent DTMF {}", digits.iter().collect::<String>()))
            }
            "status" => Ok(self.status_json()),
            "shutdown" => {
                // Hang up everything and signal the session loop to exit.
                self.phone.hangup_all();
                self.quit = true;
                Ok("Shutting down".into())
            }
            _ => Err(format!("Unknown command: {cmd}")),
        }
    }

    /// A structured snapshot of registration and active calls, returned (as a
    /// compact JSON string) for the remote `status` command. The CLI renders it
    /// as text or re-emits it as JSON depending on `--json`.
    fn status_json(&self) -> String {
        use super::app::{CallDirection, CallState, RegStatus};
        let registration = match &self.reg_status {
            RegStatus::Unknown => "unknown".to_string(),
            RegStatus::Registering => "registering".to_string(),
            RegStatus::Ok => "registered".to_string(),
            RegStatus::Failed(r) => format!("failed: {r}"),
        };
        let calls: Vec<serde_json::Value> = self
            .calls
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let direction = match c.direction {
                    CallDirection::Outgoing => "out",
                    CallDirection::Incoming => "in",
                };
                let state = match c.state {
                    CallState::Ringing => "ringing",
                    CallState::Established => "established",
                    CallState::OnHold => "on-hold",
                };
                serde_json::json!({
                    "index": i,
                    "direction": direction,
                    "peer": c.peer,
                    "state": state,
                })
            })
            .collect();
        let last_call = self.last_call.as_ref().map(|lc| {
            serde_json::json!({
                "peer": lc.peer,
                "direction": lc.direction,
                "reason": lc.reason,
                "error": lc.error,
                "duration_secs": lc.duration_secs,
                "answered": lc.answered,
            })
        });
        serde_json::json!({
            "profile": self.profile_name,
            "account": self.account_aor,
            "registration": registration,
            "muted": self.muted,
            "calls": calls,
            "last_call": last_call,
        })
        .to_string()
    }

    pub fn cycle_completion(&mut self) {
        // Already has an argument (space in input) → nothing to complete
        if self.command.input.contains(' ') {
            return;
        }

        // First Tab press: lock the prefix; subsequent presses: cycle
        let prefix = match &self.command.tab_prefix {
            Some(p) => p.clone(),
            None => {
                let p = self.command.input.clone();
                self.command.tab_prefix = Some(p.clone());
                self.command.tab_index = 0;
                p
            }
        };

        let matches: Vec<&str> = if prefix.is_empty() {
            COMMANDS.to_vec()
        } else {
            COMMANDS
                .iter()
                .filter(|c| c.starts_with(&prefix))
                .copied()
                .collect()
        };

        if matches.is_empty() {
            return;
        }

        if matches.len() == 1 {
            self.command.input = format!("{} ", matches[0]);
            self.command.tab_prefix = None;
            return;
        }

        self.command.input = matches[self.command.tab_index % matches.len()].to_string();
        self.command.tab_index += 1;
    }

    pub fn reset_tab(&mut self) {
        self.command.tab_prefix = None;
        self.command.tab_index = 0;
    }

    pub(super) fn handle_command_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Tab => {
                self.cycle_completion();
                return;
            }
            _ => self.reset_tab(),
        }
        match key.code {
            KeyCode::Esc => {
                self.command.active = false;
                self.command.input.clear();
            }
            KeyCode::Enter => {
                self.execute_command();
            }
            KeyCode::Backspace => {
                if self.command.input.is_empty() {
                    self.command.active = false;
                } else {
                    self.command.input.pop();
                }
            }
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.command.error = None;
                self.command.input.push(c);
            }
            _ => {}
        }
    }
}

/// Strip human-readable phone-number formatting from a dial target.
///
/// German-style numbers are commonly written with separators, e.g.
/// `0123-4567890` or `(0123) 4567 890`. Those separators are not valid in the
/// user part of a SIP request-URI, so a PBX can't route them. When the target
/// is a plain phone number — digits (optionally `+`/`*`/`#`) plus separators,
/// no letters, not already a SIP URI — the separators are removed. A SIP
/// username / extension name (contains letters) or an explicit `sip:`/`sips:`
/// URI is left untouched so real user parts like `john.doe` survive.
///
/// A leading `tel:` / `callto:` click-to-dial scheme (from contacts or pasted
/// links) is stripped first, then the remainder is treated as a normal target.
pub(super) fn sanitize_dial_target(input: &str) -> String {
    let trimmed = input.trim();
    let trimmed = trimmed
        .strip_prefix("tel:")
        .or_else(|| trimmed.strip_prefix("callto:"))
        .map(str::trim)
        .unwrap_or(trimmed);
    if trimmed.starts_with("sip:")
        || trimmed.starts_with("sips:")
        || trimmed.contains('@')
        || trimmed.chars().any(|c| c.is_ascii_alphabetic())
    {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .filter(|c| !(c.is_whitespace() || matches!(c, '-' | '.' | '/' | '(' | ')')))
        .collect()
}

pub(super) fn normalize_sip_uri(input: &str, account_aor: &str) -> String {
    let input = sanitize_dial_target(input);
    if input.starts_with("sip:") || input.starts_with("sips:") {
        return input;
    }
    let domain = account_aor.split_once('@').map(|x| x.1).unwrap_or("");
    if domain.is_empty() {
        input
    } else {
        format!("sip:{input}@{domain}")
    }
}

/// Valid DTMF symbols: digits, `*`, `#`, and the tones A–D (either case).
fn is_dtmf_digit(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '*' | '#' | 'A'..='D' | 'a'..='d')
}

#[cfg(test)]
mod tests {
    use super::{normalize_sip_uri, sanitize_dial_target};

    #[test]
    fn strips_separators_from_phone_numbers() {
        assert_eq!(sanitize_dial_target("0123-4567890"), "01234567890");
        assert_eq!(sanitize_dial_target("(0123) 4567 890"), "01234567890");
        assert_eq!(sanitize_dial_target("0123.4567.890"), "01234567890");
        assert_eq!(sanitize_dial_target("  0123-4567  "), "01234567");
    }

    #[test]
    fn keeps_dialable_symbols() {
        assert_eq!(sanitize_dial_target("+49 123 4567"), "+491234567");
        assert_eq!(sanitize_dial_target("*100#"), "*100#");
        assert_eq!(sanitize_dial_target("100"), "100");
    }

    #[test]
    fn strips_click_to_dial_schemes() {
        assert_eq!(sanitize_dial_target("tel:+49-30-1234567"), "+49301234567");
        assert_eq!(sanitize_dial_target("callto:0123 456"), "0123456");
        assert_eq!(sanitize_dial_target("tel:100"), "100");
    }

    #[test]
    fn leaves_sip_users_and_uris_untouched() {
        // Letters mean it's a SIP user/extension name, not a phone number.
        assert_eq!(sanitize_dial_target("john.doe"), "john.doe");
        assert_eq!(sanitize_dial_target("alice-bob"), "alice-bob");
        assert_eq!(sanitize_dial_target("user@host"), "user@host");
        assert_eq!(
            sanitize_dial_target("sip:0123-4567890@pbx"),
            "sip:0123-4567890@pbx"
        );
    }

    #[test]
    fn normalize_strips_then_builds_uri() {
        assert_eq!(
            normalize_sip_uri("0123-4567890", "me@pbx.example"),
            "sip:01234567890@pbx.example"
        );
    }

    #[test]
    fn normalize_passes_through_explicit_uri() {
        assert_eq!(
            normalize_sip_uri("sip:alice@example.com", "me@pbx.example"),
            "sip:alice@example.com"
        );
    }

    #[test]
    fn normalize_without_domain_returns_sanitized_input() {
        assert_eq!(normalize_sip_uri("0123-4567", ""), "01234567");
    }
}
