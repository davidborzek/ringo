use super::app::{CallState, InputMode, TransferMode};
use crossterm::event::{KeyCode, KeyModifiers};

impl super::app::App {
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        // Global quit — always available
        let is_quit = matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Char('c'), KeyModifiers::CONTROL)
        );
        if is_quit {
            self.phone.hangup_all();
            self.quit = true;
            return;
        }

        // Edit profile
        if matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('e'), KeyModifiers::CONTROL)
        ) && !self.has_any_call()
        {
            self.edit_profile = true;
            self.quit = true;
            return;
        }

        // Switch profile
        if matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('p'), KeyModifiers::CONTROL)
        ) {
            self.phone.hangup_all();
            self.switch_to = true;
            self.quit = true;
            return;
        }

        // History search popup captures all input
        if self.dial.mode == InputMode::HistorySearch {
            self.handle_history_search(key);
            return;
        }

        // Call history view captures navigation + action keys
        if self.call_history.show {
            self.handle_call_history_key(key);
            return;
        }

        // Transfer input modes capture all input (except quit, handled above)
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

        match key.code {
            // Ctrl+R → open history search (only when not in active call)
            KeyCode::Char('r')
                if key.modifiers == KeyModifiers::CONTROL && !self.in_active_call() =>
            {
                self.dial.draft = self.dial.input.clone();
                self.dial.query.clear();
                self.dial.selected = 0;
                self.dial.mode = InputMode::HistorySearch;
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '*' || c == '#' => {
                if self.in_active_call() {
                    self.send_dtmf(c);
                } else {
                    self.exit_history_nav();
                    self.dial_insert(c);
                }
            }
            KeyCode::Char('a') if key.modifiers == KeyModifiers::NONE => {
                if self.has_incoming_ringing() {
                    self.phone.accept();
                } else {
                    self.exit_history_nav();
                    self.dial_insert('a');
                }
            }
            KeyCode::Char('b') if key.modifiers == KeyModifiers::NONE => {
                if self.has_any_call() {
                    self.phone.hangup();
                } else {
                    self.exit_history_nav();
                    self.dial_insert('b');
                }
            }
            KeyCode::Delete => {
                if self.has_any_call() {
                    self.phone.hangup();
                } else {
                    self.dial_delete_forward();
                }
            }
            KeyCode::Char('h') if key.modifiers == KeyModifiers::NONE => {
                if self.in_active_call() {
                    self.phone.hold();
                    let idx = self.selected_call;
                    if let Some(c) = self.calls.get_mut(idx) {
                        c.state = CallState::OnHold;
                    }
                } else {
                    self.exit_history_nav();
                    self.dial_insert('h');
                }
            }
            KeyCode::Char('r') if key.modifiers == KeyModifiers::NONE => {
                if self.selected_call_on_hold() {
                    self.phone.resume();
                    let idx = self.selected_call;
                    if let Some(c) = self.calls.get_mut(idx) {
                        c.state = CallState::Established;
                    }
                } else {
                    self.exit_history_nav();
                    self.dial_insert('r');
                }
            }
            KeyCode::Char('m') if key.modifiers == KeyModifiers::NONE => {
                if self.in_active_call() {
                    self.muted = !self.muted;
                    self.phone.mute();
                } else {
                    self.exit_history_nav();
                    self.dial_insert('m');
                }
            }
            // Blind transfer — only during active call
            KeyCode::Char('t') if key.modifiers == KeyModifiers::NONE && self.in_active_call() => {
                self.transfer_mode = TransferMode::BlindInput(String::new());
            }
            // Attended transfer — only during active call (Shift+T)
            KeyCode::Char('T') if key.modifiers == KeyModifiers::SHIFT && self.in_active_call() => {
                self.transfer_mode = TransferMode::AttendedInput(String::new());
            }
            KeyCode::Backspace => {
                self.exit_history_nav();
                self.dial_backspace();
            }
            KeyCode::Enter => {
                if self.dial.mode == InputMode::HistoryNav {
                    self.dial.mode = InputMode::Dial;
                }
                if !self.dial.input.is_empty() {
                    let target = self.dial.input.clone();
                    self.phone.dial(&target);
                    crate::history::push(&mut self.dial.history, target);
                    self.dial_clear();
                }
            }
            KeyCode::Esc => {
                if self.dial.mode == InputMode::HistoryNav {
                    self.dial.mode = InputMode::Dial;
                    let draft = self.dial.draft.clone();
                    self.dial_set(draft);
                } else {
                    self.dial_clear();
                }
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::NONE => {
                self.log.show = !self.log.show;
                if self.log.show {
                    self.log.show_baresip = false;
                    self.call_history.show = false;
                }
                self.log.scroll = 0;
            }
            KeyCode::Char('l') if key.modifiers == KeyModifiers::NONE => {
                self.log.show_baresip = !self.log.show_baresip;
                if self.log.show_baresip {
                    self.log.show = false;
                    self.call_history.show = false;
                    self.refresh_baresip_log();
                    self.log.scroll = 0;
                }
            }
            KeyCode::Char('c') if key.modifiers == KeyModifiers::NONE && !self.has_any_call() => {
                self.call_history.show = !self.call_history.show;
                if self.call_history.show {
                    self.log.show = false;
                    self.log.show_baresip = false;
                    self.refresh_call_history();
                    self.log.scroll = 0;
                }
            }
            KeyCode::Left if !self.in_active_call() && key.modifiers == KeyModifiers::NONE => {
                self.dial_cursor_left();
            }
            KeyCode::Right if !self.in_active_call() && key.modifiers == KeyModifiers::NONE => {
                self.dial_cursor_right();
            }
            KeyCode::Home if !self.in_active_call() => {
                self.dial.cursor = 0;
            }
            KeyCode::End if !self.in_active_call() => {
                self.dial.cursor = self.dial.input.len();
            }
            KeyCode::Up => {
                if self.log.show || self.log.show_baresip || self.call_history.show {
                    self.log.scroll = self.log.scroll.saturating_add(1);
                } else if !self.in_active_call() {
                    match self.dial.mode {
                        InputMode::Dial => {
                            if !self.dial.history.is_empty() {
                                self.dial.draft = self.dial.input.clone();
                                self.dial.nav_idx = 0;
                                self.dial.mode = InputMode::HistoryNav;
                                let entry = self.dial.history[0].clone();
                                self.dial_set(entry);
                            }
                        }
                        InputMode::HistoryNav => {
                            if self.dial.nav_idx + 1 < self.dial.history.len() {
                                self.dial.nav_idx += 1;
                                let entry = self.dial.history[self.dial.nav_idx].clone();
                                self.dial_set(entry);
                            }
                        }
                        InputMode::HistorySearch => {}
                    }
                }
            }
            KeyCode::Down => {
                if self.log.show || self.log.show_baresip || self.call_history.show {
                    self.log.scroll = self.log.scroll.saturating_sub(1);
                } else if self.dial.mode == InputMode::HistoryNav {
                    if self.dial.nav_idx == 0 {
                        self.dial.mode = InputMode::Dial;
                        let draft = self.dial.draft.clone();
                        self.dial_set(draft);
                    } else {
                        self.dial.nav_idx -= 1;
                        let entry = self.dial.history[self.dial.nav_idx].clone();
                        self.dial_set(entry);
                    }
                }
            }
            KeyCode::Char(c) if !self.in_active_call() && key.modifiers == KeyModifiers::NONE => {
                self.exit_history_nav();
                self.dial_insert(c);
            }
            KeyCode::Tab => {
                self.switch_line();
            }
            _ => {}
        }
    }

    fn handle_history_search(&mut self, key: crossterm::event::KeyEvent) {
        let in_transfer = matches!(
            self.transfer_mode,
            TransferMode::BlindInput(_) | TransferMode::AttendedInput(_)
        );
        match key.code {
            KeyCode::Esc => {
                self.dial.mode = InputMode::Dial;
                let draft = self.dial.draft.clone();
                if in_transfer {
                    self.transfer_input_set(draft);
                } else {
                    self.dial.input = draft;
                }
            }
            KeyCode::Enter => {
                let filtered = crate::history::fuzzy_filter(&self.dial.history, &self.dial.query);
                let selected = filtered
                    .get(self.dial.selected)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| self.dial.draft.clone());
                self.dial.mode = InputMode::Dial;
                if in_transfer {
                    self.transfer_input_set(selected);
                } else {
                    self.dial_set(selected);
                }
            }
            KeyCode::Up => {
                if self.dial.selected > 0 {
                    self.dial.selected -= 1;
                }
            }
            KeyCode::Down => {
                let count =
                    crate::history::fuzzy_filter(&self.dial.history, &self.dial.query).len();
                if self.dial.selected + 1 < count {
                    self.dial.selected += 1;
                }
            }
            // Ctrl+R cycles to next match (fish-style)
            KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
                let count =
                    crate::history::fuzzy_filter(&self.dial.history, &self.dial.query).len();
                if self.dial.selected + 1 < count {
                    self.dial.selected += 1;
                }
            }
            KeyCode::Backspace => {
                self.dial.query.pop();
                self.dial.selected = 0;
            }
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.dial.query.push(c);
                self.dial.selected = 0;
            }
            _ => {}
        }
    }
}
