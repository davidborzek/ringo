use crossterm::event::{KeyCode, KeyModifiers};

use super::app::App;

pub const COMMANDS: &[&str] = &[
    "accept", "dial", "edit", "hangup", "help", "history", "hold", "log", "blog", "mute", "quit",
    "resume", "switch", "transfer", "xfer",
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
            "d" | "dial" => {
                if arg.is_empty() {
                    self.command.error = Some("Usage: dial <number>".into());
                } else {
                    self.phone.dial(arg);
                    crate::history::push(&mut self.dial.history, arg.to_string());
                }
            }
            "hangup" => {
                if self.has_any_call() {
                    self.phone.hangup();
                } else {
                    self.command.error = Some("No active call".into());
                }
            }
            "a" | "accept" => {
                if self.has_incoming_ringing() {
                    self.phone.accept();
                } else {
                    self.command.error = Some("No incoming call".into());
                }
            }
            "hold" => {
                if self.in_active_call() {
                    self.phone.hold();
                    let idx = self.selected_call;
                    if let Some(c) = self.calls.get_mut(idx) {
                        c.state = super::app::CallState::OnHold;
                    }
                } else {
                    self.command.error = Some("No active call".into());
                }
            }
            "resume" => {
                if self.selected_call_on_hold() {
                    self.phone.resume();
                    let idx = self.selected_call;
                    if let Some(c) = self.calls.get_mut(idx) {
                        c.state = super::app::CallState::Established;
                    }
                } else {
                    self.command.error = Some("No call on hold".into());
                }
            }
            "mute" => {
                if self.in_active_call() {
                    self.muted = !self.muted;
                    self.phone.mute();
                } else {
                    self.command.error = Some("No active call".into());
                }
            }
            "xfer" | "transfer" => {
                if arg.is_empty() {
                    self.command.error = Some("Usage: transfer <uri>".into());
                } else if self.in_active_call() {
                    let aor = self.account_aor.clone();
                    let uri = normalize_sip_uri(arg, &aor);
                    self.phone.transfer(&uri);
                } else {
                    self.command.error = Some("No active call".into());
                }
            }
            "log" | "e" => {
                self.log.show = !self.log.show;
                if self.log.show {
                    self.log.show_baresip = false;
                    self.call_history.show = false;
                }
                self.log.scroll = 0;
            }
            "blog" | "l" => {
                self.log.show_baresip = !self.log.show_baresip;
                if self.log.show_baresip {
                    self.log.show = false;
                    self.call_history.show = false;
                    self.refresh_baresip_log();
                }
                self.log.scroll = 0;
            }
            "history" | "c" => {
                self.call_history.show = !self.call_history.show;
                if self.call_history.show {
                    self.log.show = false;
                    self.log.show_baresip = false;
                    self.refresh_call_history();
                }
                self.log.scroll = 0;
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
            "help" | "?" => {
                self.push_log("Commands: dial <n>, hangup, accept, hold, resume, mute, transfer <uri>, log, blog, history, edit, switch, quit");
                self.log.show = true;
                self.log.show_baresip = false;
                self.call_history.show = false;
                self.log.scroll = 0;
            }
            _ => {
                self.command.error = Some(format!("Unknown command: {}", cmd));
            }
        }
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

fn normalize_sip_uri(input: &str, account_aor: &str) -> String {
    if input.starts_with("sip:") || input.starts_with("sips:") {
        return input.to_string();
    }
    let domain = account_aor.splitn(2, '@').nth(1).unwrap_or("");
    if domain.is_empty() {
        input.to_string()
    } else {
        format!("sip:{}@{}", input, domain)
    }
}
