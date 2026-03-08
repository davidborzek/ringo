use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::app::InputMode;
use super::{App, RegStatus, TransferMode};

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let title_left = Line::from(vec![
        Span::raw(" "),
        Span::styled("ringo", Style::default().fg(app.theme.accent.get())),
    ]);
    let outer = Block::default().title(title_left).borders(Borders::ALL);

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let log_visible =
        app.log.show || app.log.show_baresip || app.call_history.show || app.contacts_state.show;
    let mut constraints = vec![
        Constraint::Min(6),    // [0] calls
        Constraint::Length(1), // [1] spacer
        Constraint::Length(1), // [2] dial
        Constraint::Length(1), // [3] error reason
    ];
    if log_visible {
        constraints.push(Constraint::Min(3)); // log panel
    }
    constraints.push(Constraint::Length(1)); // status bar
    constraints.push(Constraint::Length(1)); // hints / command bar

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let len = chunks.len();
    let status_idx = len - 2;
    let cmd_idx = len - 1;

    super::call::render_calls(f, app, chunks[0]);
    super::dial::render_dial(f, app, chunks[2]);
    if let Some(reason) = &app.last_call_reason {
        f.render_widget(
            Paragraph::new(format!("  ✗ {}", reason))
                .style(Style::default().fg(app.theme.danger.get())),
            chunks[3],
        );
    }
    if log_visible {
        let log_idx = 4;
        app.log.visible_height = chunks[log_idx].height.saturating_sub(1) as usize;
        if app.contacts_state.show {
            super::contacts::render(f, app, chunks[log_idx]);
        } else if app.log.show_baresip {
            super::log::render_baresip_log(f, app, chunks[log_idx]);
        } else if app.call_history.show {
            super::call_history::render(f, app, chunks[log_idx]);
        } else {
            super::log::render_event_log(f, app, chunks[log_idx]);
        }
    }

    render_status_bar(f, app, chunks[status_idx]);
    render_command_bar(f, app, chunks[cmd_idx]);

    if app.dial.mode == InputMode::HistorySearch {
        super::dial::render_history_search(f, app, inner);
    }
}

// ─── Status Bar ──────────────────────────────────────────────────────────────

fn render_status_bar(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let sep = Span::styled(" │ ", Style::default().fg(app.theme.subtle.get()));

    let mut spans = vec![
        Span::styled(
            format!(" {} ", app.profile_name),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        sep.clone(),
    ];

    // Registration status
    let (reg_text, reg_style) = match &app.reg_status {
        RegStatus::Ok => ("● Registered", Style::default().fg(app.theme.success.get())),
        RegStatus::Failed(_) => ("✗ Failed", Style::default().fg(app.theme.danger.get())),
        RegStatus::Registering => (
            "◌ Registering",
            Style::default().fg(app.theme.attention.get()),
        ),
        RegStatus::Unknown => ("○ Connecting", Style::default().fg(app.theme.subtle.get())),
    };
    spans.push(Span::styled(reg_text, reg_style));

    // Call count
    if !app.calls.is_empty() {
        spans.push(sep.clone());
        let call_text = if app.calls.len() == 1 {
            "1 call".to_string()
        } else {
            format!("{} calls", app.calls.len())
        };
        spans.push(Span::styled(
            call_text,
            Style::default().fg(app.theme.attention.get()),
        ));
    }

    // Muted
    if app.muted {
        spans.push(sep.clone());
        spans.push(Span::styled(
            "MUTED",
            Style::default()
                .fg(app.theme.danger.get())
                .add_modifier(Modifier::BOLD),
        ));
    }

    // MWI
    if app.mwi.waiting {
        spans.push(sep.clone());
        spans.push(Span::styled(
            format!("✉ {}", app.mwi.new_messages),
            Style::default()
                .fg(app.theme.attention.get())
                .add_modifier(Modifier::BOLD),
        ));
    }

    // AOR on the right side
    let right_spans = vec![Span::styled(
        format!("{} ", app.account_aor),
        Style::default().fg(app.theme.subtle.get()),
    )];

    // Render left-aligned status and right-aligned AOR using a horizontal split
    // to prevent overlap when the left side gets long.
    let aor_width = (app.account_aor.len() + 1) as u16;
    let bar_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(aor_width)])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default()),
        bar_chunks[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right),
        bar_chunks[1],
    );
}

// ─── Command / Hint Bar ──────────────────────────────────────────────────────

fn render_command_bar(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    if app.quit_confirm {
        f.render_widget(
            Paragraph::new(" Quit? (y/n)").style(Style::default().fg(app.theme.attention.get())),
            area,
        );
        return;
    }
    if app.command.active {
        let line = Line::from(vec![
            Span::styled(":", Style::default().fg(app.theme.accent.get())),
            Span::raw(&app.command.input),
        ]);
        let cursor_x = area.x + 1 + app.command.input.len() as u16;
        f.set_cursor_position((cursor_x, area.y));
        f.render_widget(Paragraph::new(line), area);
    } else if let Some(err) = &app.command.error {
        f.render_widget(
            Paragraph::new(format!(" {}", err)).style(Style::default().fg(app.theme.danger.get())),
            area,
        );
    } else {
        render_hints(f, app, area);
    }
}

fn render_hints(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use super::app::InputMode;

    let text = match &app.transfer_mode {
        TransferMode::BlindInput(_) | TransferMode::AttendedInput(_) => {
            "[Enter] send  [Esc] cancel".to_string()
        }
        TransferMode::AttendedPending => "[X] execute transfer  [Esc] abort".to_string(),
        TransferMode::None => match app.dial.mode {
            InputMode::Dial | InputMode::HistoryNav => {
                "[Enter] dial  [Esc] cancel  [↑/↓] history  [^R] search".to_string()
            }
            InputMode::HistorySearch => String::new(),
            InputMode::Normal => {
                if app.contacts_state.show {
                    if app.contacts_state.delete_confirm.is_some() {
                        String::new() // title shows y/n prompt
                    } else if app.contacts_state.form.mode != super::app::ContactFormMode::None {
                        String::new() // form has its own hints
                    } else if app.contacts_state.search_mode {
                        "[Enter] confirm  [Esc] cancel".to_string()
                    } else {
                        "[↑/↓] nav  [Enter] dial  [/] search  [a] add  [e] edit  [d] del  [E] $EDITOR  [:] cmd  [f/Esc] close  [q] quit".to_string()
                    }
                } else if app.call_history.show {
                    if app.call_history.search_mode {
                        "[Enter] confirm  [Esc] cancel".to_string()
                    } else {
                        "[↑/↓] nav  [Enter] redial  [/] search  [d] del  [D] clear  [:] cmd  [c/Esc] close  [q] quit".to_string()
                    }
                } else if app.log.show || app.log.show_baresip {
                    "[↑/↓] scroll  [e] events  [l] log  [c] history  [:] cmd  [Esc] close  [q] quit"
                        .to_string()
                } else {
                    let mut parts: Vec<&str> = vec!["[d] dial"];
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
                        parts.push("[t] xfer  [T] att.xfer");
                    }
                    if app.calls.len() > 1 {
                        parts.push("[Tab] switch");
                    }
                    parts.push("[e] events");
                    parts.push("[l] log");
                    parts.push("[c] history");
                    parts.push("[f] contacts");
                    parts.push("[:] cmd");
                    parts.push("[q] quit");
                    parts.join("  ")
                }
            }
        },
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().fg(app.theme.subtle.get())),
        area,
    );
}
