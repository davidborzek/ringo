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
                        self.phone
                            .transfer(&super::command::normalize_sip_uri(&uri, &aor));
                    }
                    TransferMode::AttendedInput(uri) => {
                        // attended_transfer_start puts the current call on hold
                        // (SIP re-INVITE); mirror that in the UI state.
                        let idx = self.selected_call;
                        if let Some(c) = self.calls.get_mut(idx) {
                            c.state = CallState::OnHold;
                        }
                        self.phone
                            .attended_transfer_start(&super::command::normalize_sip_uri(
                                &uri, &aor,
                            ));
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
                        InputMode::HistorySearch | InputMode::Normal => {}
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
            KeyCode::Tab => {
                self.contacts_state.show = true;
                self.contacts_state.selected = 0;
                self.contacts_state.search_query.clear();
                self.contacts_state.search_mode = false;
                self.contacts_state.target = super::app::ContactPickerTarget::Transfer;
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
