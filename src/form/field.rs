use crossterm::event::KeyCode;

// ─── TextField ───────────────────────────────────────────────────────────────

pub(crate) struct TextField {
    chars: Vec<char>,
    cursor: usize,
}

impl TextField {
    pub fn new(s: &str) -> Self {
        let chars: Vec<char> = s.chars().collect();
        let cursor = chars.len();
        Self { chars, cursor }
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                self.chars.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.chars.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.chars.len() {
                    self.chars.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor < self.chars.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.chars.len(),
            _ => {}
        }
    }

    pub fn value(&self) -> String {
        self.chars.iter().collect()
    }

    /// Returns (visible_text, cursor_col_within_visible).
    pub fn render(&self, masked: bool, width: usize) -> (String, usize) {
        let display: Vec<char> = if masked {
            self.chars.iter().map(|_| '•').collect()
        } else {
            self.chars.clone()
        };
        let len = display.len();
        if len <= width {
            (display.iter().collect(), self.cursor)
        } else {
            let start = self.cursor.saturating_sub(width);
            let end = (start + width).min(len);
            (display[start..end].iter().collect(), self.cursor - start)
        }
    }
}

// ─── Field types ─────────────────────────────────────────────────────────────

pub(crate) const TRANSPORTS: &[&str] = &["default", "udp", "tcp", "tls"];
pub(crate) const ENCRYPTIONS: &[&str] = &["none", "dtls_srtp", "srtp", "srtp-mand", "zrtp"];

pub(crate) enum FieldKind {
    Text {
        tf: TextField,
        masked: bool,
    },
    Select {
        options: &'static [&'static str],
        idx: usize,
    },
    Toggle {
        value: bool,
    },
    SubMenu {
        count: usize,
    },
    Button,
}

pub(crate) struct Field {
    pub label: &'static str,
    pub required: bool,
    pub kind: FieldKind,
}

impl Field {
    pub fn text(label: &'static str, value: &str, masked: bool, required: bool) -> Self {
        Self {
            label,
            required,
            kind: FieldKind::Text {
                tf: TextField::new(value),
                masked,
            },
        }
    }
    pub fn select(label: &'static str, options: &'static [&'static str], idx: usize) -> Self {
        Self {
            label,
            required: false,
            kind: FieldKind::Select { options, idx },
        }
    }
    pub fn toggle(label: &'static str, value: bool) -> Self {
        Self {
            label,
            required: false,
            kind: FieldKind::Toggle { value },
        }
    }
    pub fn submenu(label: &'static str, count: usize) -> Self {
        Self {
            label,
            required: false,
            kind: FieldKind::SubMenu { count },
        }
    }
    pub fn button(label: &'static str) -> Self {
        Self {
            label,
            required: false,
            kind: FieldKind::Button,
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub(crate) fn get_text(f: &Field) -> String {
    if let FieldKind::Text { tf, .. } = &f.kind {
        tf.value()
    } else {
        String::new()
    }
}

pub(crate) fn get_select(f: &Field) -> usize {
    if let FieldKind::Select { idx, .. } = &f.kind {
        *idx
    } else {
        0
    }
}

pub(crate) fn get_toggle(f: &Field) -> bool {
    if let FieldKind::Toggle { value } = &f.kind {
        *value
    } else {
        false
    }
}

pub(crate) fn opt(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

pub(crate) fn transport_idx(t: Option<&str>) -> usize {
    match t {
        Some("udp") => 1,
        Some("tcp") => 2,
        Some("tls") => 3,
        _ => 0,
    }
}

pub(crate) fn enc_idx(e: Option<&str>) -> usize {
    match e {
        Some("dtls_srtp") => 1,
        Some("srtp") => 2,
        Some("srtp-mand") => 3,
        Some("zrtp") => 4,
        _ => 0,
    }
}
