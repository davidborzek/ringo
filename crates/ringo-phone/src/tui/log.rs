use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Paragraph, Wrap},
};

impl super::app::App {
    /// Lines matching the current search filter (case-insensitive substring);
    /// all lines when the query is empty.
    pub(super) fn log_filtered(&self) -> Vec<&str> {
        let q = self.log.search_query.to_lowercase();
        self.log
            .lines
            .iter()
            .filter(|l| q.is_empty() || l.to_lowercase().contains(&q))
            .map(|s| s.as_str())
            .collect()
    }

    fn log_max_scroll(&self) -> usize {
        self.log
            .content_rows
            .saturating_sub(self.log.visible_height)
    }

    fn log_page(&self) -> usize {
        self.log.visible_height.max(1)
    }

    pub(super) fn handle_log_key(&mut self, key: crossterm::event::KeyEvent) {
        // Search input captures typing until Enter/Esc.
        if self.log.search_mode {
            match key.code {
                KeyCode::Esc => {
                    self.log.search_mode = false;
                    self.log.search_query.clear();
                    self.log.scroll = 0;
                }
                KeyCode::Enter => self.log.search_mode = false,
                KeyCode::Backspace => {
                    self.log.search_query.pop();
                    self.log.scroll = 0;
                }
                KeyCode::Char(c)
                    if key.modifiers == KeyModifiers::NONE
                        || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.log.search_query.push(c);
                    self.log.scroll = 0;
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Esc => {
                if self.log.search_query.is_empty() {
                    self.log.show = false;
                } else {
                    self.log.search_query.clear();
                }
                self.log.scroll = 0;
            }
            KeyCode::Char('/') if key.modifiers == KeyModifiers::NONE => {
                self.log.search_mode = true;
                self.log.search_query.clear();
                self.log.scroll = 0;
            }
            KeyCode::Char('w') if key.modifiers == KeyModifiers::NONE => {
                self.log.wrap = !self.log.wrap;
                self.log.scroll = 0;
            }
            KeyCode::Up => {
                let max = self.log_max_scroll();
                if self.log.scroll < max {
                    self.log.scroll += 1;
                }
            }
            KeyCode::Down => {
                self.log.scroll = self.log.scroll.saturating_sub(1);
            }
            KeyCode::PageUp => {
                self.log.scroll = (self.log.scroll + self.log_page()).min(self.log_max_scroll());
            }
            KeyCode::PageDown => {
                self.log.scroll = self.log.scroll.saturating_sub(self.log_page());
            }
            KeyCode::Char('g') if key.modifiers == KeyModifiers::NONE => {
                self.log.scroll = self.log_max_scroll();
            }
            KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
                self.log.scroll = 0;
            }
            _ => {}
        }
    }
}

/// Render the (filtered) log into a modal's content area (frame/title/footer are
/// drawn by the caller). The view is anchored to the bottom; `scroll` counts
/// display rows back from the end (0 = following the tail). Long lines are
/// truncated unless `wrap` is on.
pub(super) fn render_logs(f: &mut Frame, app: &mut super::app::App, area: Rect) {
    let width = (area.width as usize).max(1);
    let visible = area.height as usize;

    let lines: Vec<String> = app.log_filtered().iter().map(|s| s.to_string()).collect();
    // Total display rows: wrapped lines span ceil(len / width) rows each.
    let total: usize = if app.log.wrap {
        lines
            .iter()
            .map(|l| l.chars().count().div_ceil(width).max(1))
            .sum()
    } else {
        lines.len()
    };
    app.log.content_rows = total;
    app.log.visible_height = visible;

    let max_scroll = total.saturating_sub(visible);
    if app.log.scroll > max_scroll {
        app.log.scroll = max_scroll;
    }
    let top = max_scroll.saturating_sub(app.log.scroll) as u16;

    let text: Vec<Line> = lines.into_iter().map(Line::from).collect();
    let mut para = Paragraph::new(text)
        .style(Style::default().fg(app.theme.subtle.get()))
        .scroll((top, 0));
    if app.log.wrap {
        para = para.wrap(Wrap { trim: false });
    }
    f.render_widget(para, area);
}
