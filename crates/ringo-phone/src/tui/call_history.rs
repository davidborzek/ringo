use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use super::app::{Call, CallDirection, CallHistoryEntry};

/// Truncate `s` to `width` display columns (char-aware, appending `…` when it
/// doesn't fit), otherwise left-pad it to `width` so the next column aligns.
fn fit(s: &str, width: usize) -> String {
    if s.chars().count() > width {
        let mut t: String = s.chars().take(width.saturating_sub(1)).collect();
        t.push('…');
        t
    } else {
        format!("{s:<width$}")
    }
}

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
            headers: call.header_views.clone(),
        };
        if let Ok(mut line) = serde_json::to_string(&entry) {
            line.push('\n');
            if let Err(e) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| f.write_all(line.as_bytes()))
            {
                crate::rlog!(Warn, "call history write failed: {}", e);
            }
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
        self.call_history.delete_confirm = None;
        self.call_history.detail = false;
    }

    pub(super) fn clear_call_history(&mut self) {
        self.call_history.entries.clear();
        self.call_history.selected = 0;
        if let Some(path) = &self.call_history.path {
            if let Err(e) = std::fs::write(path, "") {
                crate::rlog!(Warn, "call history clear failed: {}", e);
            }
        }
    }

    pub(super) fn handle_call_history_key(&mut self, key: crossterm::event::KeyEvent) {
        use super::app::HistoryDelete;

        // Detail view — Esc / i / q close it.
        if self.call_history.detail {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('i') | KeyCode::Char('q')
            ) {
                self.call_history.detail = false;
            }
            return;
        }

        // Delete confirmation captures all input until y (confirm) or anything
        // else (cancel).
        if let Some(kind) = self.call_history.delete_confirm {
            let mut do_delete = false;
            let mut close = false;
            match key.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                    self.confirm_yes = !self.confirm_yes;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => do_delete = true,
                KeyCode::Enter if self.confirm_yes => do_delete = true,
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    close = true;
                }
                _ => {}
            }
            if do_delete {
                match kind {
                    HistoryDelete::One => {
                        let indices = self.call_history.filtered_indices(&self.contacts);
                        if let Some(&real_idx) = indices.get(self.call_history.selected) {
                            self.call_history.entries.remove(real_idx);
                            self.rewrite_call_history_file();
                            let new_len = self.call_history.filtered_indices(&self.contacts).len();
                            if self.call_history.selected >= new_len && new_len > 0 {
                                self.call_history.selected = new_len - 1;
                            }
                        }
                    }
                    HistoryDelete::All => self.clear_call_history(),
                }
            }
            if do_delete || close {
                self.call_history.delete_confirm = None;
                self.confirm_yes = false;
            }
            return;
        }

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

        let indices = self.call_history.filtered_indices(&self.contacts);
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
            KeyCode::PageUp => {
                self.call_history.selected = self.call_history.selected.saturating_sub(10);
            }
            KeyCode::PageDown => {
                if filtered_len > 0 {
                    self.call_history.selected =
                        (self.call_history.selected + 10).min(filtered_len - 1);
                }
            }
            KeyCode::Enter => {
                if let Some(&real_idx) = indices.get(self.call_history.selected) {
                    let peer = self.call_history.entries[real_idx].peer.clone();
                    self.dial_set(peer);
                    self.dial.mode = super::app::InputMode::Dial;
                    self.call_history.show = false;
                    self.call_history.search_query.clear();
                    self.call_history.search_mode = false;
                }
            }
            // d → confirm deletion of the selected entry
            KeyCode::Char('d') if key.modifiers == KeyModifiers::NONE => {
                if indices.get(self.call_history.selected).is_some() {
                    self.call_history.delete_confirm = Some(HistoryDelete::One);
                    self.confirm_yes = false;
                }
            }
            // D → confirm clearing the whole history
            KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT && filtered_len > 0 => {
                self.call_history.delete_confirm = Some(HistoryDelete::All);
                self.confirm_yes = false;
            }
            // i → show details (peer + captured headers) for the selected entry
            KeyCode::Char('i')
                if key.modifiers == KeyModifiers::NONE
                    && indices.get(self.call_history.selected).is_some() =>
            {
                self.call_history.detail = true;
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
    let indices = app.call_history.filtered_indices(&app.contacts);
    let total = app.call_history.entries.len();
    let filtered_len = indices.len();

    if app.call_history.detail {
        render_history_detail(f, app, area, &indices);
        return;
    }

    let sel = if filtered_len > 0 {
        app.call_history.selected.min(filtered_len - 1)
    } else {
        0
    };

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

    let search_footer = [("Enter", "confirm"), ("Esc", "clear")];
    let nav_footer = [
        ("↑↓", "nav"),
        ("PgUp/PgDn", "page"),
        ("Enter", "redial"),
        ("i", "details"),
        ("/", "search"),
        ("d", "del"),
        ("D", "clear"),
        ("Esc", "close"),
    ];
    let footer: &[super::ui::Hint] = if app.call_history.search_mode {
        &search_footer
    } else {
        &nav_footer
    };

    f.render_widget(Clear, area);
    let block = Block::default()
        .title(title)
        .title_alignment(ratatui::layout::Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Size the visible window to the actual list area (inner minus the — possibly
    // wrapped — footer) so the selected row is always kept in view.
    let footer_h = super::ui::hint_rows(footer, inner.width).min(inner.height.saturating_sub(1));
    let visible = inner.height.saturating_sub(footer_h) as usize;
    let scroll = if sel < visible { 0 } else { sel - visible + 1 };

    // Peer column width adapts to the modal, reserving room for arrow (3), gaps,
    // duration (9) and timestamp (19); the rest goes to the peer (truncated).
    let peer_w = (area.width as usize)
        .saturating_sub(2 + 3 + 2 + 9 + 2 + 19)
        .clamp(12, 60);

    let items: Vec<ListItem> = indices
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(fi, &ri)| call_history_item(&app.call_history.entries[ri], app, fi == sel, peer_w))
        .collect();

    let list_area = Rect::new(inner.x, inner.y, inner.width, visible as u16);
    f.render_widget(List::new(items), list_area);
    if footer_h > 0 {
        super::ui::render_hint_bar(
            f,
            Rect::new(inner.x, inner.y + visible as u16, inner.width, footer_h),
            footer,
            &app.theme,
        );
    }
    super::ui::render_scrollbar(f, &app.theme, area, filtered_len, visible, scroll);
}

fn call_history_item<'a>(
    e: &'a super::app::CallHistoryEntry,
    app: &super::app::App,
    selected: bool,
    peer_w: usize,
) -> ListItem<'a> {
    let (arrow, dir_style) = if e.dir == "outgoing" {
        ("↗", Style::default().fg(app.theme.accent.get()))
    } else {
        ("↙", Style::default().fg(app.theme.success.get()))
    };

    // On the selected row the background is `subtle`, so the subtle-grey columns
    // (duration, timestamp) would be invisible — fall back to the default fg there.
    let dim = if selected {
        Style::default()
    } else {
        Style::default().fg(app.theme.subtle.get())
    };
    let dur_style = if e.duration == "missed" || e.duration == "no answer" {
        Style::default().fg(app.theme.danger.get())
    } else {
        dim
    };

    let peer_display = match crate::contacts::resolve_name(&app.contacts, &e.peer) {
        Some(name) => format!("{} ({})", name, e.peer),
        None => e.peer.clone(),
    };

    let line = Line::from(vec![
        Span::styled(format!(" {} ", arrow), dir_style),
        Span::raw(fit(&peer_display, peer_w)),
        Span::styled(format!("  {:<9}", e.duration), dur_style),
        Span::styled(format!("  {}", e.ts), dim),
    ]);

    let item = ListItem::new(line);
    if selected {
        item.style(Style::default().bg(app.theme.subtle.get()))
    } else {
        item
    }
}

/// Per-entry detail overlay: call metadata + the captured inbound-header views.
fn render_history_detail(f: &mut Frame, app: &super::app::App, area: Rect, indices: &[usize]) {
    let accent = Style::default().fg(app.theme.accent.get());
    let subtle = Style::default().fg(app.theme.subtle.get());
    let sel = app
        .call_history
        .selected
        .min(indices.len().saturating_sub(1));
    let entry = indices
        .get(sel)
        .and_then(|&i| app.call_history.entries.get(i));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Call details ")
        .title_alignment(Alignment::Center)
        .border_style(subtle);
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let mut lines = Vec::new();
    if let Some(e) = entry {
        let field = |k: &str, v: &str| {
            Line::from(vec![
                Span::styled(format!("  {k:<10}"), subtle),
                Span::styled(v.to_string(), Style::default()),
            ])
        };
        lines.push(field("When", &e.ts));
        lines.push(field("Direction", &e.dir));
        lines.push(field("Peer", &e.peer));
        lines.push(field("Duration", &e.duration));
        lines.push(Line::from(""));
        if e.headers.is_empty() {
            lines.push(Line::from(Span::styled("  No headers captured.", subtle)));
        } else {
            for (label, value) in &e.headers {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {label}  "), accent),
                    Span::styled(value.clone(), Style::default()),
                ]));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  Esc / i  back", subtle)));
    } else {
        lines.push(Line::from(Span::styled("  No entry.", subtle)));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
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
                headers: Vec::new(),
            })
            .collect();
        CallHistoryState {
            path: None,
            entries,
            show: true,
            selected: 0,
            search_query: query.to_string(),
            search_mode: false,
            delete_confirm: None,
            detail: false,
        }
    }

    #[test]
    fn empty_query_returns_all_indices() {
        let s = make_state(&["sip:alice@example.com", "sip:bob@example.com"], "");
        assert_eq!(s.filtered_indices(&[]), vec![0, 1]);
    }

    #[test]
    fn query_filters_by_peer() {
        let s = make_state(&["sip:alice@example.com", "sip:bob@example.com"], "alice");
        assert_eq!(s.filtered_indices(&[]), vec![0]);
    }

    #[test]
    fn query_is_case_insensitive() {
        let s = make_state(&["sip:Alice@Example.com"], "alice");
        assert_eq!(s.filtered_indices(&[]), vec![0]);
    }

    #[test]
    fn no_match_returns_empty() {
        let s = make_state(&["sip:alice@example.com"], "zzz");
        assert_eq!(s.filtered_indices(&[]), vec![] as Vec<usize>);
    }

    #[test]
    fn empty_entries_returns_empty() {
        let s = make_state(&[], "alice");
        assert_eq!(s.filtered_indices(&[]), vec![] as Vec<usize>);
    }
}
