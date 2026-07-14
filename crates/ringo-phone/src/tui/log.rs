use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{Frame, layout::Rect, style::Style, text::Line, widgets::Paragraph};

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
        self.log_filtered()
            .len()
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

/// Render the (filtered) log lines into a modal's content area (frame/title/footer
/// drawn by the caller). Newest lines sit at the bottom; `scroll` counts lines back
/// from the end.
pub(super) fn render_logs(f: &mut Frame, app: &super::app::App, area: Rect) {
    let visible = area.height as usize;
    let lines = app.log_filtered();
    let total = lines.len();
    let skip = app.log.scroll.min(total.saturating_sub(visible));
    let end = total.saturating_sub(skip);
    let start = end.saturating_sub(visible);

    let text: Vec<Line> = lines[start..end].iter().map(|s| Line::from(*s)).collect();

    f.render_widget(
        Paragraph::new(text).style(Style::default().fg(app.theme.subtle.get())),
        area,
    );
}
