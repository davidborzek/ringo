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

        // Command bar captures all input when active
        if self.command.active {
            self.handle_command_key(key);
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

        // Transfer input modes capture all input
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

    // ─── Normal Mode ─────────────────────────────────────────────────────────

    fn handle_normal_key(&mut self, key: crossterm::event::KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            // Quit (with confirmation)
            KeyCode::Char('q') if !ctrl => {
                self.quit_confirm = true;
            }

            // Edit profile (only when no calls)
            KeyCode::Char('e') if ctrl && !self.has_any_call() => {
                self.edit_profile = true;
                self.quit = true;
            }

            // Switch profile
            KeyCode::Char('p') if ctrl => {
                self.phone.hangup_all();
                self.switch_to = true;
                self.quit = true;
            }

            // Command bar
            KeyCode::Char(':') => {
                self.command.active = true;
                self.command.input.clear();
                self.command.error = None;
            }

            // Enter dial mode
            KeyCode::Char('d') if !ctrl => {
                self.dial.mode = InputMode::Dial;
                self.command.error = None;
            }

            // Accept incoming call
            KeyCode::Char('a') if !ctrl && self.has_incoming_ringing() => {
                self.phone.accept();
            }

            // Hangup
            KeyCode::Char('b') if !ctrl && self.has_any_call() => {
                self.phone.hangup();
            }
            KeyCode::Delete if self.has_any_call() => {
                self.phone.hangup();
            }

            // Hold
            KeyCode::Char('h') if !ctrl && self.in_active_call() => {
                self.phone.hold();
                let idx = self.selected_call;
                if let Some(c) = self.calls.get_mut(idx) {
                    c.state = CallState::OnHold;
                }
            }

            // Resume
            KeyCode::Char('r') if !ctrl && self.selected_call_on_hold() => {
                self.phone.resume();
                let idx = self.selected_call;
                if let Some(c) = self.calls.get_mut(idx) {
                    c.state = CallState::Established;
                }
            }

            // Mute
            KeyCode::Char('m') if !ctrl && self.in_active_call() => {
                self.muted = !self.muted;
                self.phone.mute();
            }

            // Blind transfer
            KeyCode::Char('t') if key.modifiers == KeyModifiers::NONE && self.in_active_call() => {
                self.transfer_mode = TransferMode::BlindInput(String::new());
            }

            // Attended transfer (Shift+T)
            KeyCode::Char('T') if key.modifiers == KeyModifiers::SHIFT && self.in_active_call() => {
                self.transfer_mode = TransferMode::AttendedInput(String::new());
            }

            // DTMF during active call
            KeyCode::Char(c)
                if (c.is_ascii_digit() || c == '*' || c == '#') && self.in_active_call() =>
            {
                self.send_dtmf(c);
            }

            // Switch call
            KeyCode::Tab => {
                self.switch_line();
            }

            // Toggle event log
            KeyCode::Char('e') if !ctrl => {
                self.log.show = !self.log.show;
                if self.log.show {
                    self.log.show_baresip = false;
                    self.call_history.show = false;
                }
                self.log.scroll = 0;
            }

            // Toggle baresip log
            KeyCode::Char('l') if !ctrl => {
                self.log.show_baresip = !self.log.show_baresip;
                if self.log.show_baresip {
                    self.log.show = false;
                    self.call_history.show = false;
                    self.refresh_baresip_log();
                    self.log.scroll = 0;
                }
            }

            // Toggle call history
            KeyCode::Char('c') if !ctrl => {
                self.call_history.show = !self.call_history.show;
                if self.call_history.show {
                    self.log.show = false;
                    self.log.show_baresip = false;
                    self.refresh_call_history();
                    self.log.scroll = 0;
                }
            }

            // History search
            KeyCode::Char('r') if ctrl => {
                self.dial.draft = self.dial.input.clone();
                self.dial.query.clear();
                self.dial.selected = 0;
                self.dial.mode = InputMode::HistorySearch;
            }

            // Scroll log views
            KeyCode::Up if self.log.show || self.log.show_baresip || self.call_history.show => {
                self.log.scroll = self.log.scroll.saturating_add(1);
            }
            KeyCode::Down if self.log.show || self.log.show_baresip || self.call_history.show => {
                self.log.scroll = self.log.scroll.saturating_sub(1);
            }

            _ => {}
        }
    }

    // ─── Dial Mode ───────────────────────────────────────────────────────────

    fn handle_dial_key(&mut self, key: crossterm::event::KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                if self.dial.mode == InputMode::HistoryNav {
                    let draft = self.dial.draft.clone();
                    self.dial_set(draft);
                }
                self.dial.mode = InputMode::Normal;
                self.dial_clear();
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
                self.dial.mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                if self.dial.mode == InputMode::HistoryNav {
                    self.dial.mode = InputMode::Dial;
                    let draft = self.dial.draft.clone();
                    self.dial_set(draft);
                } else if self.dial.input.is_empty() {
                    self.dial.mode = InputMode::Normal;
                } else {
                    self.dial_backspace();
                }
            }
            KeyCode::Delete => {
                self.dial_delete_forward();
            }

            // History search
            KeyCode::Char('r') if ctrl => {
                self.dial.draft = self.dial.input.clone();
                self.dial.query.clear();
                self.dial.selected = 0;
                self.dial.mode = InputMode::HistorySearch;
            }

            // History navigation
            KeyCode::Up => match self.dial.mode {
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
                _ => {}
            },
            KeyCode::Down => {
                if self.dial.mode == InputMode::HistoryNav {
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

            // Cursor movement
            KeyCode::Left if key.modifiers == KeyModifiers::NONE => {
                self.dial_cursor_left();
            }
            KeyCode::Right if key.modifiers == KeyModifiers::NONE => {
                self.dial_cursor_right();
            }
            KeyCode::Home => {
                self.dial.cursor = 0;
            }
            KeyCode::End => {
                self.dial.cursor = self.dial.input.len();
            }

            // Type characters
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                if self.dial.mode == InputMode::HistoryNav {
                    self.dial.mode = InputMode::Dial;
                }
                self.dial_insert(c);
            }

            _ => {}
        }
    }

    // ─── Command Bar ─────────────────────────────────────────────────────────

    fn handle_command_key(&mut self, key: crossterm::event::KeyEvent) {
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

    // ─── History Search ──────────────────────────────────────────────────────

    fn handle_history_search(&mut self, key: crossterm::event::KeyEvent) {
        let in_transfer = matches!(
            self.transfer_mode,
            TransferMode::BlindInput(_) | TransferMode::AttendedInput(_)
        );
        match key.code {
            KeyCode::Esc => {
                self.dial.mode = if in_transfer {
                    InputMode::Dial
                } else {
                    InputMode::Dial
                };
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
