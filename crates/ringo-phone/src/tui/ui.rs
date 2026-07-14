use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::config::Theme;

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

    // Fixed layout; all secondary views (Logs, Help, Call history, Contacts) are
    // centered modal overlays drawn on top.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // [0] calls
            Constraint::Length(1), // [1] spacer
            Constraint::Length(1), // [2] dial
            Constraint::Length(1), // [3] error reason
            Constraint::Length(1), // [4] status bar
            Constraint::Length(1), // [5] hints / command bar
        ])
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

    render_status_bar(f, app, chunks[4]);
    render_command_bar(f, app, chunks[5]);

    // Modal overlays (mutually exclusive — `close_overlays` keeps only one open).
    if app.log.show {
        let title = if app.log.search_mode {
            format!("Logs  / {}_", app.log.search_query)
        } else if !app.log.search_query.is_empty() {
            format!(
                "Logs  /{}  ({})",
                app.log.search_query,
                app.log_filtered().len()
            )
        } else {
            "Logs".to_string()
        };
        let footer = if app.log.search_mode {
            "type to filter   Enter confirm   Esc clear"
        } else {
            "↑↓ / PgUp PgDn scroll   g/G top/bottom   / search   Esc close"
        };
        let content = render_modal(f, &app.theme, 80, 80, &title, footer);
        app.log.visible_height = content.height as usize;
        super::log::render_logs(f, app, content);
    } else if app.help_show {
        render_help(f, app);
    } else if app.call_history.show {
        super::call_history::render(f, app, centered_rect(area, 80, 80));
        if let Some(kind) = app.call_history.delete_confirm {
            let q = match kind {
                super::app::HistoryDelete::One => "Delete the selected call?".to_string(),
                super::app::HistoryDelete::All => "Clear the entire call history?".to_string(),
            };
            render_confirm_popup(f, &app.theme, &q, "Delete", app.confirm_yes);
        }
    } else if app.contacts_state.show {
        super::contacts::render(f, app, centered_rect(area, 80, 80));
        if let Some(ci) = app.contacts_state.delete_confirm {
            let name = app.contacts.get(ci).map(|c| c.name.as_str()).unwrap_or("?");
            let q = format!("Delete \"{}\"?", name);
            render_confirm_popup(f, &app.theme, &q, "Delete", app.confirm_yes);
        }
    }

    if app.dial.mode == InputMode::HistorySearch {
        super::dial::render_history_search(f, app, inner);
    }
}

/// A centered rectangle sized to `w_pct`×`h_pct` of `area`.
fn centered_rect(area: Rect, w_pct: u16, h_pct: u16) -> Rect {
    let w = ((area.width as u32 * w_pct as u32 / 100) as u16).clamp(4, area.width);
    let h = ((area.height as u32 * h_pct as u32 / 100) as u16).clamp(3, area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// Draw a centered modal (Clear + rounded border + centered title) sized to
/// `w_pct`×`h_pct` of the screen, with a subtle `footer` hint on the last inner
/// row. Returns the content area above the footer for the caller to fill.
/// Generic on purpose so Logs/Help (and later History/Contacts) share one frame.
fn render_modal(
    f: &mut Frame,
    theme: &Theme,
    w_pct: u16,
    h_pct: u16,
    title: &str,
    footer: &str,
) -> Rect {
    let rect = centered_rect(f.area(), w_pct, h_pct);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .title_alignment(Alignment::Center);
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    if !footer.is_empty() && inner.height >= 2 {
        let fy = inner.y + inner.height - 1;
        f.render_widget(
            Paragraph::new(format!("  {}", footer)).style(Style::default().fg(theme.subtle.get())),
            Rect::new(inner.x, fy, inner.width, 1),
        );
        return Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
    }
    inner
}

/// A small centered yes/no confirmation popup with `Cancel` / destructive
/// buttons. `yes` highlights the destructive button; the caller owns that state
/// (`App::confirm_yes`) and the key handling.
fn render_confirm_popup(
    f: &mut Frame,
    theme: &Theme,
    question: &str,
    confirm_label: &str,
    yes: bool,
) {
    let area = f.area();
    let w = (question.chars().count() as u16 + 8)
        .clamp(36, 60)
        .min(area.width);
    let h = 5u16.min(area.height);
    let rect = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.danger.get()))
        .title("Confirm")
        .title_alignment(Alignment::Center);
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    if inner.height < 3 {
        return;
    }

    f.render_widget(
        Paragraph::new(question).alignment(Alignment::Center),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let button = |label: &str, selected: bool, danger: bool| {
        let style = if selected {
            let base = if danger {
                Style::default().fg(theme.danger.get())
            } else {
                Style::default()
            };
            base.add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::default().fg(theme.subtle.get())
        };
        Span::styled(format!("  {}  ", label), style)
    };
    let buttons = Line::from(vec![
        button("Cancel", !yes, false),
        Span::raw("   "),
        button(confirm_label, yes, true),
    ]);
    f.render_widget(
        Paragraph::new(buttons).alignment(Alignment::Center),
        Rect::new(inner.x, inner.y + 2, inner.width, 1),
    );
}

/// Static key/command reference shown in the Help modal.
fn render_help(f: &mut Frame, app: &App) {
    let content = render_modal(f, &app.theme, 60, 70, "Help", "Esc close");
    let accent = Style::default().fg(app.theme.accent.get());
    let subtle = Style::default().fg(app.theme.subtle.get());
    let row = |key: &str, desc: &str| {
        Line::from(vec![
            Span::styled(format!("  {:<12}", key), accent),
            Span::styled(desc.to_string(), Style::default()),
        ])
    };
    let lines = vec![
        Line::from(Span::styled("  Keys", subtle)),
        row("d", "dial"),
        row("a", "accept incoming"),
        row("b / Del", "hang up"),
        row("h / r", "hold / resume"),
        row("m", "mute"),
        row("t / T", "blind / attended transfer"),
        row("Tab", "switch active call"),
        row("l", "logs"),
        row("c", "call history"),
        row("f", "contacts"),
        row("?", "this help"),
        row(":", "command"),
        row("q", "quit"),
        Line::from(""),
        Line::from(Span::styled("  Commands (:)", subtle)),
        Line::from(Span::styled(
            "  dial <n>  hangup  accept  hold  resume  mute",
            Style::default(),
        )),
        Line::from(Span::styled(
            "  dtmf <digits>  transfer <uri>  log  history",
            Style::default(),
        )),
        Line::from(Span::styled(
            "  contacts  edit  switch  quit",
            Style::default(),
        )),
    ];
    f.render_widget(Paragraph::new(lines), content);
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
        RegStatus::Unknown => (
            "○ Connecting",
            Style::default().fg(app.theme.attention.get()),
        ),
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
            "[Enter] send  [Tab] contacts  [Esc] cancel".to_string()
        }
        TransferMode::AttendedPending => "[X] execute transfer  [Esc] abort".to_string(),
        TransferMode::None => match app.dial.mode {
            InputMode::Dial | InputMode::HistoryNav => {
                "[Enter] dial  [Tab] contacts  [Esc] cancel  [↑/↓] history  [^R] search".to_string()
            }
            InputMode::HistorySearch => String::new(),
            InputMode::Normal => {
                // Overlay-specific hints live in each modal's own footer; the base
                // hint line always shows the main call actions.
                {
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
                    parts.push("[l] logs");
                    parts.push("[c] history");
                    parts.push("[f] contacts");
                    parts.push("[?] help");
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
