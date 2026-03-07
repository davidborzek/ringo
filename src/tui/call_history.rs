use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

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
            // Command bar
            KeyCode::Char(':') => {
                self.command.active = true;
                self.command.input.clear();
                self.command.error = None;
            }
            KeyCode::Char('q') if key.modifiers == KeyModifiers::NONE => {
                self.phone.hangup_all();
                self.quit = true;
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

pub(super) fn render(f: &mut Frame, app: &super::app::App, area: Rect) {
    let indices = app.call_history.filtered_indices();
    let total = app.call_history.entries.len();
    let filtered_len = indices.len();
    let visible = area.height.saturating_sub(2) as usize;

    let sel = if filtered_len > 0 {
        app.call_history.selected.min(filtered_len - 1)
    } else {
        0
    };
    let scroll = if sel < visible { 0 } else { sel - visible + 1 };

    let items: Vec<ListItem> = indices
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(fi, &ri)| {
            let item = call_history_item(&app.call_history.entries[ri], app);
            if fi == sel {
                item.style(Style::default().bg(app.theme.subtle.get()))
            } else {
                item
            }
        })
        .collect();

    let accent = Style::default().fg(app.theme.accent.get());
    let subtle = Style::default().fg(app.theme.subtle.get());
    let title: Line = if app.call_history.search_mode {
        Line::from(vec![
            Span::styled("Call History", accent),
            Span::styled(
                format!("  / {}_", app.call_history.search_query),
                Style::default().fg(app.theme.attention.get()),
            ),
        ])
    } else if !app.call_history.search_query.is_empty() {
        Line::from(vec![
            Span::styled("Call History", accent),
            Span::styled(
                format!(
                    "  /{} ({}/{})",
                    app.call_history.search_query, filtered_len, total
                ),
                subtle,
            ),
        ])
    } else if total == 0 {
        Line::from(vec![
            Span::styled("Call History", accent),
            Span::styled("  (empty)", subtle),
        ])
    } else {
        Line::from(vec![
            Span::styled("Call History", accent),
            Span::styled(
                format!(
                    " ({}/{})",
                    if filtered_len > 0 { sel + 1 } else { 0 },
                    filtered_len
                ),
                subtle,
            ),
        ])
    };

    f.render_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::TOP)),
        area,
    );
}

fn call_history_item<'a>(
    e: &'a super::app::CallHistoryEntry,
    app: &super::app::App,
) -> ListItem<'a> {
    let (arrow, dir_style) = if e.dir == "outgoing" {
        ("↗", Style::default().fg(app.theme.accent.get()))
    } else {
        ("↙", Style::default().fg(app.theme.success.get()))
    };

    let dur_style = if e.duration == "missed" || e.duration == "no answer" {
        Style::default().fg(app.theme.danger.get())
    } else {
        Style::default().fg(app.theme.subtle.get())
    };

    let line = Line::from(vec![
        Span::styled(format!(" {} ", arrow), dir_style),
        Span::raw(format!("{:<45}", e.peer)),
        Span::styled(format!("{:<11}", e.duration), dur_style),
        Span::styled(
            format!("  {}", e.ts),
            Style::default().fg(app.theme.subtle.get()),
        ),
    ]);

    ListItem::new(line)
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
