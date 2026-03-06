use super::app::{CallState, InputMode, TransferMode};
use crossterm::event::{KeyCode, KeyModifiers};

impl super::app::App {
    pub(super) fn handle_transfer_input(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.transfer_mode = TransferMode::None;
                self.dial.mode = InputMode::Dial;
            }
            KeyCode::Backspace => {
                if self.dial.mode == InputMode::HistoryNav {
                    self.dial.mode = InputMode::Dial;
                    let draft = self.dial.draft.clone();
                    self.transfer_input_set(draft);
                } else {
                    match &mut self.transfer_mode {
                        TransferMode::BlindInput(s) | TransferMode::AttendedInput(s) => {
                            s.pop();
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Enter => {
                self.dial.mode = InputMode::Dial;
                let old = std::mem::replace(&mut self.transfer_mode, TransferMode::None);
                let aor = self.account_aor.clone();
                match old {
                    TransferMode::BlindInput(uri) => {
                        self.phone.transfer(&normalize_sip_uri(&uri, &aor));
                    }
                    TransferMode::AttendedInput(uri) => {
                        // baresip puts the current call on hold immediately
                        let idx = self.selected_call;
                        if let Some(c) = self.calls.get_mut(idx) {
                            c.state = CallState::OnHold;
                        }
                        self.phone
                            .attended_transfer_start(&normalize_sip_uri(&uri, &aor));
                        self.transfer_mode = TransferMode::AttendedPending;
                    }
                    _ => {}
                }
            }
            KeyCode::Up => {
                if !self.dial.history.is_empty() {
                    match self.dial.mode {
                        InputMode::Dial => {
                            let current = self.transfer_input_get();
                            self.dial.draft = current;
                            self.dial.nav_idx = 0;
                            self.dial.mode = InputMode::HistoryNav;
                            let entry = self.dial.history[0].clone();
                            self.transfer_input_set(entry);
                        }
                        InputMode::HistoryNav => {
                            if self.dial.nav_idx + 1 < self.dial.history.len() {
                                self.dial.nav_idx += 1;
                                let entry = self.dial.history[self.dial.nav_idx].clone();
                                self.transfer_input_set(entry);
                            }
                        }
                        InputMode::HistorySearch => {}
                    }
                }
            }
            KeyCode::Down => {
                if self.dial.mode == InputMode::HistoryNav {
                    if self.dial.nav_idx == 0 {
                        self.dial.mode = InputMode::Dial;
                        let draft = self.dial.draft.clone();
                        self.transfer_input_set(draft);
                    } else {
                        self.dial.nav_idx -= 1;
                        let entry = self.dial.history[self.dial.nav_idx].clone();
                        self.transfer_input_set(entry);
                    }
                }
            }
            KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
                let current = self.transfer_input_get();
                self.dial.draft = current;
                self.dial.query.clear();
                self.dial.selected = 0;
                self.dial.mode = InputMode::HistorySearch;
            }
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.dial.mode = InputMode::Dial; // exit history nav on typing
                match &mut self.transfer_mode {
                    TransferMode::BlindInput(s) | TransferMode::AttendedInput(s) => {
                        s.push(c);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub(super) fn handle_transfer_pending(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('X') if key.modifiers == KeyModifiers::SHIFT => {
                self.phone.attended_transfer_exec();
                self.transfer_mode = TransferMode::None;
            }
            KeyCode::Esc => {
                self.phone.attended_transfer_abort();
                self.transfer_mode = TransferMode::None;
            }
            KeyCode::Tab => {
                self.switch_line();
            }
            _ => {}
        }
    }

    pub(super) fn transfer_input_get(&self) -> String {
        match &self.transfer_mode {
            TransferMode::BlindInput(s) | TransferMode::AttendedInput(s) => s.clone(),
            _ => String::new(),
        }
    }

    pub(super) fn transfer_input_set(&mut self, value: String) {
        match &mut self.transfer_mode {
            TransferMode::BlindInput(s) | TransferMode::AttendedInput(s) => *s = value,
            _ => {}
        }
    }
}

/// Ensure the input is a full SIP URI.
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
