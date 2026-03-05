use crossterm::event::{KeyCode, KeyModifiers};

use super::app::{Call, CallDirection, CallHistoryEntry};

impl super::app::App {
    pub(super) fn append_call_history(&self, call: &Call) {
        use std::io::Write;
        let Some(path) = &self.call_history.path else {
            return;
        };

        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let dir = match call.direction {
            CallDirection::Outgoing => "outgoing",
            CallDirection::Incoming => "incoming",
        };
        let (duration, duration_secs) = match call.started_at {
            Some(start) => {
                let s = start.elapsed().as_secs();
                (
                    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60),
                    s,
                )
            }
            None => {
                let label = match call.direction {
                    CallDirection::Incoming => "missed",
                    CallDirection::Outgoing => "no answer",
                };
                (label.to_string(), 0)
            }
        };

        let entry = CallHistoryEntry {
            ts,
            dir: dir.to_string(),
            peer: call.peer.clone(),
            duration,
            duration_secs,
        };
        if let Ok(mut line) = serde_json::to_string(&entry) {
            line.push('\n');
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| f.write_all(line.as_bytes()));
        }
    }

    pub(super) fn refresh_call_history(&mut self) {
        let Some(path) = &self.call_history.path else {
            return;
        };
        if let Ok(content) = std::fs::read_to_string(path) {
            self.call_history.entries = content
                .lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();
            self.call_history.entries.reverse();
        }
        self.call_history.search_query.clear();
        self.call_history.search_mode = false;
        self.call_history.selected = 0;
    }

    pub(super) fn clear_call_history(&mut self) {
        self.call_history.entries.clear();
        self.call_history.selected = 0;
        if let Some(path) = &self.call_history.path {
            let _ = std::fs::write(path, "");
        }
    }

    pub(super) fn handle_call_history_key(&mut self, key: crossterm::event::KeyEvent) {
        // Search mode: capture typing
        if self.call_history.search_mode {
            match key.code {
                KeyCode::Esc => {
                    self.call_history.search_mode = false;
                    self.call_history.search_query.clear();
                    self.call_history.selected = 0;
                }
                KeyCode::Enter => {
                    self.call_history.search_mode = false;
                }
                KeyCode::Backspace => {
                    self.call_history.search_query.pop();
                    self.call_history.selected = 0;
                }
                KeyCode::Char(c)
                    if key.modifiers == KeyModifiers::NONE
                        || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.call_history.search_query.push(c);
                    self.call_history.selected = 0;
                }
                _ => {}
            }
            return;
        }

        let indices = self.call_history.filtered_indices();
        let filtered_len = indices.len();

        match key.code {
            KeyCode::Esc => {
                if !self.call_history.search_query.is_empty() {
                    self.call_history.search_query.clear();
                    self.call_history.selected = 0;
                } else {
                    self.call_history.show = false;
                }
            }
            KeyCode::Char('c') if key.modifiers == KeyModifiers::NONE => {
                self.call_history.search_query.clear();
                self.call_history.search_mode = false;
                self.call_history.show = false;
            }
            KeyCode::Char('/') if key.modifiers == KeyModifiers::NONE => {
                self.call_history.search_mode = true;
                self.call_history.search_query.clear();
                self.call_history.selected = 0;
            }
            KeyCode::Char('g') if key.modifiers == KeyModifiers::NONE => {
                self.call_history.selected = 0;
            }
            KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
                if filtered_len > 0 {
                    self.call_history.selected = filtered_len - 1;
                }
            }
            KeyCode::Up => {
                if self.call_history.selected > 0 {
                    self.call_history.selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.call_history.selected + 1 < filtered_len {
                    self.call_history.selected += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(&real_idx) = indices.get(self.call_history.selected) {
                    let peer = self.call_history.entries[real_idx].peer.clone();
                    self.dial_set(peer);
                    self.call_history.show = false;
                    self.call_history.search_query.clear();
                    self.call_history.search_mode = false;
                }
            }
            // d → delete selected filtered entry
            KeyCode::Char('d') if key.modifiers == KeyModifiers::NONE => {
                if let Some(&real_idx) = indices.get(self.call_history.selected) {
                    self.call_history.entries.remove(real_idx);
                    self.rewrite_call_history_file();
                    let new_len = self.call_history.filtered_indices().len();
                    if self.call_history.selected >= new_len && new_len > 0 {
                        self.call_history.selected = new_len - 1;
                    }
                }
            }
            // D → clear entire history
            KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => {
                self.clear_call_history();
            }
            _ => {}
        }
    }

    fn rewrite_call_history_file(&self) {
        let Some(path) = &self.call_history.path else {
            return;
        };
        let content = self
            .call_history
            .entries
            .iter()
            .rev()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(
            path,
            if content.is_empty() {
                content
            } else {
                content + "\n"
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::app::{CallHistoryEntry, CallHistoryState};

    fn make_state(peers: &[&str], query: &str) -> CallHistoryState {
        let entries = peers
            .iter()
            .map(|p| CallHistoryEntry {
                ts: "2024-01-01 12:00:00".into(),
                dir: "outgoing".into(),
                peer: p.to_string(),
                duration: "00:01:00".into(),
                duration_secs: 60,
            })
            .collect();
        CallHistoryState {
            path: None,
            entries,
            show: true,
            selected: 0,
            search_query: query.to_string(),
            search_mode: false,
        }
    }

    #[test]
    fn empty_query_returns_all_indices() {
        let s = make_state(&["sip:alice@example.com", "sip:bob@example.com"], "");
        assert_eq!(s.filtered_indices(), vec![0, 1]);
    }

    #[test]
    fn query_filters_by_peer() {
        let s = make_state(&["sip:alice@example.com", "sip:bob@example.com"], "alice");
        assert_eq!(s.filtered_indices(), vec![0]);
    }

    #[test]
    fn query_is_case_insensitive() {
        let s = make_state(&["sip:Alice@Example.com"], "alice");
        assert_eq!(s.filtered_indices(), vec![0]);
    }

    #[test]
    fn no_match_returns_empty() {
        let s = make_state(&["sip:alice@example.com"], "zzz");
        assert_eq!(s.filtered_indices(), vec![] as Vec<usize>);
    }

    #[test]
    fn empty_entries_returns_empty() {
        let s = make_state(&[], "alice");
        assert_eq!(s.filtered_indices(), vec![] as Vec<usize>);
    }
}
