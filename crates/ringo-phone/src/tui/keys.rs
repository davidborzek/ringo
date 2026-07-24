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

        // Quit confirmation popup captures all input.
        if self.quit_confirm {
            let mut do_quit = false;
            match key.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                    self.confirm_yes = !self.confirm_yes;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => do_quit = true,
                KeyCode::Enter if self.confirm_yes => do_quit = true,
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.quit_confirm = false;
                    self.confirm_yes = false;
                }
                _ => {}
            }
            if do_quit {
                self.phone.hangup_all();
                self.quit = true;
            }
            return;
        }

        // Switch-profile confirmation popup (back to the picker).
        if self.switch_confirm {
            let mut do_switch = false;
            match key.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                    self.confirm_yes = !self.confirm_yes;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => do_switch = true,
                KeyCode::Enter if self.confirm_yes => do_switch = true,
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.switch_confirm = false;
                    self.confirm_yes = false;
                }
                _ => {}
            }
            if do_switch {
                self.phone.hangup_all();
                self.switch_to = true;
                self.quit = true;
            }
            return;
        }

        // Global: q → quit confirm, : → command bar (except during text input)
        let in_text_input = self.command.active
            || self.dial.mode == InputMode::Dial
            || self.dial.mode == InputMode::HistoryNav
            || self.dial.mode == InputMode::HistorySearch
            || matches!(
                self.transfer_mode,
                TransferMode::BlindInput(_) | TransferMode::AttendedInput(_)
            )
            || (self.contacts_state.show && self.contacts_state.search_mode)
            || (self.contacts_state.show
                && self.contacts_state.form.mode != super::app::ContactFormMode::None)
            || (self.contacts_state.show && self.contacts_state.delete_confirm.is_some())
            || (self.call_history.show && self.call_history.search_mode)
            || (self.call_history.show && self.call_history.delete_confirm.is_some())
            || (self.log.show && self.log.search_mode);
        if !in_text_input {
            match key.code {
                KeyCode::Char('q') if key.modifiers == KeyModifiers::NONE => {
                    self.quit_confirm = true;
                    // Preselect Quit so `q` then Enter exits quickly.
                    self.confirm_yes = true;
                    return;
                }
                KeyCode::Char(':') => {
                    self.command.active = true;
                    self.command.input.clear();
                    self.command.error = None;
                    return;
                }
                // Overlay toggles work from anywhere: open the target, switch
                // straight from another overlay, or close it if it's already up.
                KeyCode::Char('l') if key.modifiers == KeyModifiers::NONE => {
                    let open = !self.log.show;
                    self.close_overlays();
                    if open {
                        self.log.show = true;
                        self.refresh_log();
                    }
                    return;
                }
                KeyCode::Char('c') if key.modifiers == KeyModifiers::NONE => {
                    let open = !self.call_history.show;
                    self.close_overlays();
                    if open {
                        self.call_history.show = true;
                        self.refresh_call_history();
                    }
                    return;
                }
                KeyCode::Char('f') if key.modifiers == KeyModifiers::NONE => {
                    let open = !self.contacts_state.show;
                    self.close_overlays();
                    if open {
                        self.contacts_state.show = true;
                        self.contacts_state.selected = 0;
                        self.contacts_state.search_query.clear();
                        self.contacts_state.search_mode = false;
                        self.contacts_state.target = super::app::ContactPickerTarget::Dial;
                    }
                    return;
                }
                KeyCode::Char('?') if key.modifiers == KeyModifiers::NONE => {
                    let open = !self.help_show;
                    self.close_overlays();
                    self.help_show = open;
                    return;
                }
                _ => {}
            }
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

        // Contacts overlay
        if self.contacts_state.show {
            self.handle_contacts_key(key);
            return;
        }

        // Call history view
        if self.call_history.show {
            self.handle_call_history_key(key);
            return;
        }

        // Help modal — Esc / ? / q close it, everything else is swallowed.
        if self.help_show {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                self.help_show = false;
            }
            return;
        }

        // Logs modal
        if self.log.show {
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
            KeyCode::Char('e') if ctrl && !self.has_any_call() => {
                self.edit_profile = true;
                self.quit = true;
            }
            KeyCode::Char('p') if ctrl => {
                self.phone.hangup_all();
                self.switch_to = true;
                self.quit = true;
            }
            KeyCode::Esc => {
                self.switch_confirm = true;
                // Preselect Switch so Esc then Enter goes back quickly.
                self.confirm_yes = true;
            }
            KeyCode::Char('d') if !ctrl => {
                self.dial.mode = InputMode::Dial;
                self.command.error = None;
            }
            KeyCode::Char('a') if !ctrl && self.has_incoming_ringing() => {
                self.accept_incoming();
            }
            KeyCode::Char('b') if !ctrl && self.has_any_call() => {
                self.hangup_selected();
            }
            KeyCode::Delete if self.has_any_call() => {
                self.hangup_selected();
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
            KeyCode::Tab if self.calls.len() > 1 => {
                self.switch_line();
            }
            KeyCode::Tab => {
                self.close_overlays();
                self.contacts_state.show = true;
                self.contacts_state.selected = 0;
                self.contacts_state.search_query.clear();
                self.contacts_state.search_mode = false;
                self.contacts_state.target = super::app::ContactPickerTarget::Dial;
            }
            // l / c / f / ? (open/switch/close overlays) are handled globally
            // above so they also work from inside another overlay.
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
