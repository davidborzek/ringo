use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crossterm::event::{KeyCode, KeyModifiers};

use super::app::{InputMode, TransferMode};

impl super::app::App {
    pub(super) fn handle_dial_key(&mut self, key: crossterm::event::KeyEvent) {
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
            KeyCode::Char('r') if ctrl => {
                self.dial.draft = self.dial.input.clone();
                self.dial.query.clear();
                self.dial.selected = 0;
                self.dial.mode = InputMode::HistorySearch;
            }
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

    pub(super) fn handle_history_search(&mut self, key: crossterm::event::KeyEvent) {
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

    /// Insert a character at the current cursor position and advance cursor.
    pub fn dial_insert(&mut self, c: char) {
        self.dial.input.insert(self.dial.cursor, c);
        self.dial.cursor += c.len_utf8();
    }

    /// Delete the character before the cursor (Backspace).
    pub fn dial_backspace(&mut self) {
        if self.dial.cursor == 0 {
            return;
        }
        let new_cursor = self.dial.input[..self.dial.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.dial.input.remove(new_cursor);
        self.dial.cursor = new_cursor;
    }

    /// Delete the character at the cursor (Delete key).
    pub fn dial_delete_forward(&mut self) {
        if self.dial.cursor < self.dial.input.len() {
            self.dial.input.remove(self.dial.cursor);
        }
    }

    /// Set the dial input and move cursor to the end.
    pub fn dial_set(&mut self, s: String) {
        self.dial.cursor = s.len();
        self.dial.input = s;
    }

    /// Clear the dial input and reset cursor.
    pub fn dial_clear(&mut self) {
        self.dial.input.clear();
        self.dial.cursor = 0;
    }

    pub fn dial_cursor_left(&mut self) {
        if self.dial.cursor == 0 {
            return;
        }
        self.dial.cursor = self.dial.input[..self.dial.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
    }

    pub fn dial_cursor_right(&mut self) {
        if self.dial.cursor >= self.dial.input.len() {
            return;
        }
        let c = self.dial.input[self.dial.cursor..].chars().next().unwrap();
        self.dial.cursor += c.len_utf8();
    }
}

pub(super) fn render_dial(f: &mut Frame, app: &super::app::App, area: Rect) {
    use super::app::TransferMode;

    let line = match &app.transfer_mode {
        TransferMode::BlindInput(s) => Line::from(vec![
            Span::styled("  Xfer → : ", Style::default().fg(app.theme.transfer.get())),
            Span::styled(
                format!("{}_", s),
                Style::default().fg(app.theme.transfer.get()),
            ),
        ]),
        TransferMode::AttendedInput(s) => Line::from(vec![
            Span::styled("  Att. → : ", Style::default().fg(app.theme.transfer.get())),
            Span::styled(
                format!("{}_", s),
                Style::default().fg(app.theme.transfer.get()),
            ),
        ]),
        TransferMode::AttendedPending => Line::from(vec![Span::styled(
            "  Attended: call ringing…",
            Style::default().fg(app.theme.attention.get()),
        )]),
        TransferMode::None => match app.dial.mode {
            InputMode::Normal => {
                if app.in_active_call() {
                    Line::from(vec![
                        Span::styled("  DTMF: ", Style::default().fg(app.theme.accent.get())),
                        Span::styled(&app.dial.dtmf, Style::default().fg(app.theme.accent.get())),
                    ])
                } else {
                    Line::default()
                }
            }
            InputMode::HistoryNav => Line::from(vec![
                Span::styled("  Hist: ", Style::default().fg(app.theme.attention.get())),
                Span::raw(format!("{}_", app.dial.input)),
            ]),
            InputMode::Dial => {
                let cursor = app.dial.cursor.min(app.dial.input.len());
                let before = &app.dial.input[..cursor];
                let after = &app.dial.input[cursor..];
                let cursor_x = area.x + 8 + before.chars().count() as u16;
                f.set_cursor_position((cursor_x, area.y));
                Line::from(vec![
                    Span::styled("  Dial: ", Style::default().fg(app.theme.accent.get())),
                    Span::raw(before),
                    Span::raw(after),
                ])
            }
            InputMode::HistorySearch => Line::default(),
        },
    };
    f.render_widget(Paragraph::new(line), area);
}

pub(super) fn render_history_search(f: &mut Frame, app: &super::app::App, area: Rect) {
    let filtered = crate::history::fuzzy_filter(&app.dial.history, &app.dial.query);

    let max_visible: usize = 8;
    let visible = filtered.len().min(max_visible);
    let popup_h = (visible as u16 + 3)
        .max(4)
        .min(area.height.saturating_sub(2));
    let popup_w = area.width.saturating_sub(6).max(30);
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + area.height.saturating_sub(popup_h + 2);
    let popup_area = Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" History  (↑↓ navigate · Enter select · Esc cancel) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.accent.get()));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    f.render_widget(
        Paragraph::new(format!(" / {}_", app.dial.query))
            .style(Style::default().fg(app.theme.attention.get())),
        chunks[0],
    );

    let scroll = app.dial.selected.saturating_sub(visible.saturating_sub(1));
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(i, entry)| {
            if i == app.dial.selected {
                ListItem::new(format!(" {}", entry)).style(
                    Style::default()
                        .fg(app.theme.attention.get())
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(format!(" {}", entry)).style(Style::default())
            }
        })
        .collect();

    f.render_widget(List::new(items), chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::super::app::App;
    use crate::phone::Phone;

    struct NoopPhone;
    impl Phone for NoopPhone {
        fn register(&self, _: &str, _: u32) {}
        fn dial(&self, _: &str) {}
        fn hangup(&self) {}
        fn hangup_all(&self) {}
        fn accept(&self) {}
        fn hold(&self) {}
        fn resume(&self) {}
        fn mute(&self) {}
        fn send_dtmf(&self, _: char) {}
        fn switch_line(&self, _: usize) {}
        fn transfer(&self, _: &str) {}
        fn attended_transfer_start(&self, _: &str) {}
        fn attended_transfer_exec(&self) {}
        fn attended_transfer_abort(&self) {}
        fn add_header(&self, _: &str, _: &str) {}
    }

    fn test_app() -> App {
        App::new(
            "test".into(),
            "sip:test@example.com".into(),
            None,
            None,
            false,
            Box::new(NoopPhone),
            crate::config::Theme::default(),
        )
    }

    #[test]
    fn insert_appends_and_advances_cursor() {
        let mut app = test_app();
        app.dial_insert('a');
        app.dial_insert('b');
        assert_eq!(app.dial.input, "ab");
        assert_eq!(app.dial.cursor, 2);
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut app = test_app();
        app.dial_insert('a');
        app.dial_insert('b');
        app.dial_backspace();
        assert_eq!(app.dial.input, "a");
        assert_eq!(app.dial.cursor, 1);
    }

    #[test]
    fn backspace_at_start_is_noop() {
        let mut app = test_app();
        app.dial_backspace();
        assert_eq!(app.dial.input, "");
        assert_eq!(app.dial.cursor, 0);
    }

    #[test]
    fn delete_forward_removes_char_at_cursor() {
        let mut app = test_app();
        app.dial_set("abc".into());
        app.dial.cursor = 1;
        app.dial_delete_forward();
        assert_eq!(app.dial.input, "ac");
        assert_eq!(app.dial.cursor, 1);
    }

    #[test]
    fn cursor_left_and_right() {
        let mut app = test_app();
        app.dial_set("abc".into());
        app.dial_cursor_left();
        assert_eq!(app.dial.cursor, 2);
        app.dial_cursor_right();
        assert_eq!(app.dial.cursor, 3);
    }

    #[test]
    fn cursor_does_not_go_out_of_bounds() {
        let mut app = test_app();
        app.dial_set("ab".into());
        app.dial_cursor_right(); // already at end
        assert_eq!(app.dial.cursor, 2);
        app.dial.cursor = 0;
        app.dial_cursor_left(); // already at start
        assert_eq!(app.dial.cursor, 0);
    }

    #[test]
    fn dial_clear_resets_input_and_cursor() {
        let mut app = test_app();
        app.dial_set("hello".into());
        app.dial_clear();
        assert_eq!(app.dial.input, "");
        assert_eq!(app.dial.cursor, 0);
    }

    #[test]
    fn insert_at_middle_cursor() {
        let mut app = test_app();
        app.dial_set("ac".into());
        app.dial.cursor = 1;
        app.dial_insert('b');
        assert_eq!(app.dial.input, "abc");
        assert_eq!(app.dial.cursor, 2);
    }
}
