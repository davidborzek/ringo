use super::app::InputMode;


impl super::app::App {
    pub fn exit_history_nav(&mut self) {
        if self.dial.mode == InputMode::HistoryNav {
            self.dial.mode = InputMode::Dial;
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

#[cfg(test)]
mod tests {
    use super::super::app::App;
    use crate::phone::Phone;

    struct NoopPhone;
    impl Phone for NoopPhone {
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
