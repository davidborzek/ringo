use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, List, ListItem},
};

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
        format!("Events ↑{} (↓ scroll down)", app.log.scroll)
    } else {
        "Events  (↑/↓ scroll)".to_string()
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

    let title = if app.log.baresip_path.is_some() {
        if app.log.scroll > 0 {
            format!("baresip.log ↑{} (↓ scroll down, [l] back)", app.log.scroll)
        } else {
            "baresip.log  (↑/↓ scroll, [l] back)".to_string()
        }
    } else {
        "baresip.log  (no log path)".to_string()
    };

    f.render_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::TOP)),
        area,
    );
}
