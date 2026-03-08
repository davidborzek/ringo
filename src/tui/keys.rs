use super::app::{CallState, InputMode, TransferMode};
use crossterm::event::{KeyCode, KeyModifiers};

impl super::app::App {
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        // Ctrl+C → quit immediately (always, regardless of mode)
        if matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('c'), KeyModifiers::CONTROL)
        ) {
            self.phone.hangup_all();
            self.quit = true;
            return;
        }

        // Quit confirmation captures all input
        if self.quit_confirm {
            match key.code {
                KeyCode::Char('y') => {
                    self.phone.hangup_all();
                    self.quit = true;
                }
                _ => self.quit_confirm = false,
            }
            return;
        }

        // Command bar
        if self.command.active {
            self.handle_command_key(key);
            return;
        }

        // History search popup
        if self.dial.mode == InputMode::HistorySearch {
            self.handle_history_search(key);
            return;
        }

        // Call history view
        if self.call_history.show {
            self.handle_call_history_key(key);
            return;
        }

        // Event log / baresip log view
        if self.log.show || self.log.show_baresip {
            self.handle_log_key(key);
            return;
        }

        // Transfer input modes
        match &self.transfer_mode {
            TransferMode::BlindInput(_) | TransferMode::AttendedInput(_) => {
                self.handle_transfer_input(key);
                return;
            }
            TransferMode::AttendedPending => {
                self.handle_transfer_pending(key);
                return;
            }
            TransferMode::None => {}
        }

        // Dial / HistoryNav mode
        if self.dial.mode == InputMode::Dial || self.dial.mode == InputMode::HistoryNav {
            self.handle_dial_key(key);
            return;
        }

        // Normal mode
        self.handle_normal_key(key);
    }

    fn handle_normal_key(&mut self, key: crossterm::event::KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') if !ctrl => {
                self.quit_confirm = true;
            }
            KeyCode::Char('e') if ctrl && !self.has_any_call() => {
                self.edit_profile = true;
                self.quit = true;
            }
            KeyCode::Char('p') if ctrl => {
                self.phone.hangup_all();
                self.switch_to = true;
                self.quit = true;
            }
            KeyCode::Char(':') => {
                self.command.active = true;
                self.command.input.clear();
                self.command.error = None;
            }
            KeyCode::Char('d') if !ctrl => {
                self.dial.mode = InputMode::Dial;
                self.command.error = None;
            }
            KeyCode::Char('a') if !ctrl && self.has_incoming_ringing() => {
                self.phone.accept();
            }
            KeyCode::Char('b') if !ctrl && self.has_any_call() => {
                self.phone.hangup();
            }
            KeyCode::Delete if self.has_any_call() => {
                self.phone.hangup();
            }
            KeyCode::Char('h') if !ctrl && self.in_active_call() => {
                self.phone.hold();
                let idx = self.selected_call;
                if let Some(c) = self.calls.get_mut(idx) {
                    c.state = CallState::OnHold;
                }
            }
            KeyCode::Char('r') if !ctrl && self.selected_call_on_hold() => {
                self.phone.resume();
                let idx = self.selected_call;
                if let Some(c) = self.calls.get_mut(idx) {
                    c.state = CallState::Established;
                }
            }
            KeyCode::Char('m') if !ctrl && self.in_active_call() => {
                self.muted = !self.muted;
                self.phone.mute();
            }
            KeyCode::Char('t') if key.modifiers == KeyModifiers::NONE && self.in_active_call() => {
                self.transfer_mode = TransferMode::BlindInput(String::new());
            }
            KeyCode::Char('T') if key.modifiers == KeyModifiers::SHIFT && self.in_active_call() => {
                self.transfer_mode = TransferMode::AttendedInput(String::new());
            }
            KeyCode::Char(c)
                if (c.is_ascii_digit() || c == '*' || c == '#') && self.in_active_call() =>
            {
                self.send_dtmf(c);
            }
            KeyCode::Tab => {
                self.switch_line();
            }
            KeyCode::Char('e') if !ctrl => {
                self.log.show = true;
                self.log.show_baresip = false;
                self.call_history.show = false;
                self.log.scroll = 0;
            }
            KeyCode::Char('l') if !ctrl => {
                self.log.show_baresip = true;
                self.log.show = false;
                self.call_history.show = false;
                self.refresh_baresip_log();
                self.log.scroll = 0;
            }
            KeyCode::Char('c') if !ctrl => {
                self.call_history.show = true;
                self.log.show = false;
                self.log.show_baresip = false;
                self.refresh_call_history();
                self.log.scroll = 0;
            }
            KeyCode::Char('r') if ctrl => {
                self.dial.draft = self.dial.input.clone();
                self.dial.query.clear();
                self.dial.selected = 0;
                self.dial.mode = InputMode::HistorySearch;
            }
            _ => {}
        }
    }
}
