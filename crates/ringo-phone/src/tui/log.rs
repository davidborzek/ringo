use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, List, ListItem},
};

impl super::app::App {
    fn log_max_scroll(&self) -> usize {
        let total = if self.log.show_baresip {
            self.log.baresip_lines.len()
        } else {
            self.log.entries.len()
        };
        total.saturating_sub(self.log.visible_height)
    }

    pub(super) fn handle_log_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.log.show = false;
                self.log.show_baresip = false;
                self.log.scroll = 0;
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::NONE => {
                if self.log.show {
                    self.log.show = false;
                    self.log.scroll = 0;
                } else {
                    self.log.show = true;
                    self.log.show_baresip = false;
                    self.log.scroll = 0;
                }
            }
            KeyCode::Char('l') if key.modifiers == KeyModifiers::NONE => {
                if self.log.show_baresip {
                    self.log.show_baresip = false;
                    self.log.scroll = 0;
                } else {
                    self.log.show_baresip = true;
                    self.log.show = false;
                    self.refresh_baresip_log();
                    self.log.scroll = 0;
                }
            }
            KeyCode::Char('c') if key.modifiers == KeyModifiers::NONE => {
                self.log.show = false;
                self.log.show_baresip = false;
                self.call_history.show = true;
                self.refresh_call_history();
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

pub(super) fn render_event_log(f: &mut Frame, app: &super::app::App, area: Rect) {
    let visible = area.height.saturating_sub(1) as usize;
    let total = app.log.entries.len();
    let skip = app.log.scroll.min(total.saturating_sub(visible));
    let end = total.saturating_sub(skip);
    let start = end.saturating_sub(visible);

    let items: Vec<ListItem> = app
        .log
        .entries
        .iter()
        .skip(start)
        .take(visible)
        .map(|s| ListItem::new(s.as_str()).style(Style::default().fg(app.theme.subtle.get())))
        .collect();

    let title = if app.log.scroll > 0 {
        format!("Events ↑{}", app.log.scroll)
    } else {
        "Events".to_string()
    };
    f.render_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::TOP)),
        area,
    );
}

pub(super) fn render_baresip_log(f: &mut Frame, app: &super::app::App, area: Rect) {
    let visible = area.height.saturating_sub(1) as usize;
    let lines = &app.log.baresip_lines;
    let total = lines.len();
    let skip = app.log.scroll.min(total.saturating_sub(visible));
    let end = total.saturating_sub(skip);
    let start = end.saturating_sub(visible);

    let items: Vec<ListItem> = lines[start..end]
        .iter()
        .map(|s| ListItem::new(s.as_str()).style(Style::default().fg(app.theme.subtle.get())))
        .collect();

    let title = if app.log.baresip_path.is_none() {
        "baresip.log  (no log path)".to_string()
    } else if app.log.scroll > 0 {
        format!("baresip.log ↑{}", app.log.scroll)
    } else {
        "baresip.log".to_string()
    };

    f.render_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::TOP)),
        area,
    );
}
