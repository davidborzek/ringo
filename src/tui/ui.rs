use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::app::InputMode;
use super::{App, RegStatus, TransferMode};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let title_left = Line::from(vec![
        Span::raw(" "),
        Span::styled("ringo", Style::default().fg(app.theme.accent.get())),
        Span::styled(
            format!(" — {} ({}) ", app.profile_name, app.account_aor),
            Style::default().fg(app.theme.subtle.get()),
        ),
    ]);
    let mut title_right_spans = vec![];
    if app.mwi.waiting {
        title_right_spans.push(Span::styled(
            format!(" ✉ {} ", app.mwi.new_messages),
            Style::default()
                .fg(app.theme.attention.get())
                .add_modifier(Modifier::BOLD),
        ));
    }
    let reg_text = reg_text(&app.reg_status);
    let reg_style = reg_style(&app.reg_status, app);
    title_right_spans.push(Span::styled(format!(" {} ", reg_text), reg_style));

    let outer = Block::default()
        .title(title_left)
        .title(
            ratatui::widgets::block::Title::from(Line::from(title_right_spans))
                .alignment(Alignment::Right),
        )
        .borders(Borders::ALL);

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let log_visible = app.log.show || app.log.show_baresip || app.call_history.show;
    let mut constraints = vec![
        Constraint::Min(6),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ];
    if log_visible {
        constraints.push(Constraint::Min(3));
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    super::call::render_calls(f, app, chunks[0]);
    super::dial::render_dial(f, app, chunks[2]);
    if let Some(reason) = &app.last_call_reason {
        f.render_widget(
            Paragraph::new(format!("  ✗ {}", reason))
                .style(Style::default().fg(app.theme.danger.get())),
            chunks[3],
        );
    }
    render_keys(f, app, chunks[4]);
    if log_visible {
        if app.log.show_baresip {
            super::log::render_baresip_log(f, app, chunks[5]);
        } else if app.call_history.show {
            super::call_history::render(f, app, chunks[5]);
        } else {
            super::log::render_event_log(f, app, chunks[5]);
        }
    }

    if app.dial.mode == InputMode::HistorySearch {
        super::dial::render_history_search(f, app, inner);
    }
}

fn render_keys(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let text = match &app.transfer_mode {
        TransferMode::BlindInput(_) | TransferMode::AttendedInput(_) => {
            "[Enter] send  [Esc] cancel".to_string()
        }
        TransferMode::AttendedPending => "[X] execute transfer  [Esc] abort".to_string(),
        TransferMode::None => {
            if app.log.show_baresip {
                "[l] hide  [↑/↓] scroll  [q] quit".to_string()
            } else if app.log.show {
                "[e] hide  [l] baresip log  [c] call history  [↑/↓] scroll  [q] quit".to_string()
            } else if app.call_history.show {
                String::new()
            } else {
                let mut parts: Vec<&str> = vec!["[Enter] dial"];
                if app.has_incoming_ringing() {
                    parts.push("[a] accept");
                }
                if app.has_any_call() {
                    parts.push("[b] hangup");
                }
                if app.in_active_call() {
                    parts.push("[h] hold");
                }
                if app.selected_call_on_hold() {
                    parts.push("[r] resume");
                }
                if app.in_active_call() {
                    parts.push("[m] mute");
                }
                if app.in_active_call() {
                    parts.push("[t] transfer  [T] att.xfer");
                }
                if app.calls.len() > 1 {
                    parts.push("[Tab] switch");
                }
                parts.push("[e] event log");
                parts.push("[l] baresip log");
                if !app.has_any_call() {
                    parts.push("[c] call history");
                    parts.push("[ctrl+e] edit profile");
                }
                parts.push("[q] quit");
                parts.join("  ")
            }
        }
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().fg(app.theme.subtle.get())),
        area,
    );
}

fn reg_text(status: &RegStatus) -> &'static str {
    match status {
        RegStatus::Unknown => "○ Connecting",
        RegStatus::Registering => "◌ Registering",
        RegStatus::Ok => "● Registered",
        RegStatus::Failed(_) => "✗ Registration Failed",
    }
}

fn reg_style(status: &RegStatus, app: &App) -> Style {
    match status {
        RegStatus::Ok => Style::default().fg(app.theme.success.get()),
        RegStatus::Failed(_) => Style::default().fg(app.theme.danger.get()),
        RegStatus::Registering => Style::default().fg(app.theme.attention.get()),
        RegStatus::Unknown => Style::default().fg(app.theme.subtle.get()),
    }
}
