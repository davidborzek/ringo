use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use super::{App, CallDirection, CallHistoryEntry, CallState, InputMode, RegStatus, TransferMode};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let reg_text = reg_text(&app.reg_status);
    let reg_style = reg_style(&app.reg_status, app);

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

    // Inner layout — log chunk only added when visible so no space is reserved
    let log_visible = app.log.show || app.log.show_baresip || app.call_history.show;
    let mut constraints = vec![
        Constraint::Min(6),    // calls list — expands when log is hidden
        Constraint::Length(1), // spacer
        Constraint::Length(1), // dial input
        Constraint::Length(1), // spacer
        Constraint::Length(1), // keybindings
    ];
    if log_visible {
        constraints.push(Constraint::Min(3));
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    render_calls(f, app, chunks[0]);
    render_dial(f, app, chunks[2]);
    render_keys(f, app, chunks[4]);
    if log_visible {
        if app.log.show_baresip {
            render_baresip_log(f, app, chunks[5]);
        } else if app.call_history.show {
            render_call_history(f, app, chunks[5]);
        } else {
            render_log(f, app, chunks[5]);
        }
    }

    if app.dial.mode == InputMode::HistorySearch {
        render_history_search(f, app, inner);
    }
}

fn render_calls(f: &mut Frame, app: &App, area: Rect) {
    if app.calls.is_empty() {
        let w = Paragraph::new("  (no active calls)")
            .style(Style::default().fg(app.theme.subtle.get()))
            .block(
                Block::default()
                    .title(Span::styled("CALLS", Style::default().fg(app.theme.subtle.get()))),
            );
        f.render_widget(w, area);
        return;
    }

    let items: Vec<ListItem> = app
        .calls
        .iter()
        .enumerate()
        .map(|(i, call)| {
            let arrow = match call.direction {
                CallDirection::Outgoing => "↗",
                CallDirection::Incoming => "↙",
            };
            let dir = match call.direction {
                CallDirection::Outgoing => "outgoing",
                CallDirection::Incoming => "incoming",
            };

            let selected = i == app.selected_call;
            let on_hold = call.state == CallState::OnHold;

            let base_style = if selected {
                Style::default()
                    .fg(app.theme.attention.get())
                    .add_modifier(Modifier::BOLD)
            } else if on_hold {
                Style::default().fg(app.theme.subtle.get())
            } else {
                Style::default()
            };

            let (state_str, state_style) = match &call.state {
                CallState::Ringing => (
                    "RINGING".to_string(),
                    Style::default()
                        .fg(app.theme.attention.get())
                        .add_modifier(Modifier::BOLD),
                ),
                CallState::OnHold => (
                    "ON HOLD".to_string(),
                    Style::default()
                        .fg(app.theme.subtle.get())
                        .add_modifier(Modifier::DIM),
                ),
                CallState::Established => {
                    let s = call.started_at.map(|t| t.elapsed().as_secs()).unwrap_or(0);
                    (
                        format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60),
                        Style::default().fg(app.theme.success.get()),
                    )
                }
            };

            let marker = if selected { "►" } else { " " };

            // Color the direction arrow by call type when the row isn't
            // overridden by selection (attention) or hold (subtle).
            let arrow_style = if selected || on_hold {
                base_style
            } else if call.direction == CallDirection::Outgoing {
                Style::default().fg(app.theme.accent.get())
            } else {
                Style::default().fg(app.theme.success.get())
            };

            let line = Line::from(vec![
                Span::styled(
                    format!(" {} [{}] ", marker, i + 1),
                    base_style,
                ),
                Span::styled(format!("{} ", arrow), arrow_style),
                Span::styled(
                    format!("{:<8}  {:<40}  ", dir, call.peer),
                    base_style,
                ),
                Span::styled(state_str, state_style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(Span::styled("CALLS", Style::default().fg(app.theme.subtle.get()))),
    );
    f.render_widget(list, area);
}

fn render_dial(f: &mut Frame, app: &App, area: Rect) {
    let mute_indicator = if app.muted {
        Span::styled(" [MUTED]", Style::default().fg(app.theme.danger.get()))
    } else {
        Span::raw("")
    };

    let line = match &app.transfer_mode {
        TransferMode::BlindInput(s) => Line::from(vec![
            Span::styled("  Xfer → : ", Style::default().fg(app.theme.transfer.get())),
            Span::styled(format!("{}_", s), Style::default().fg(app.theme.transfer.get())),
            mute_indicator,
        ]),
        TransferMode::AttendedInput(s) => Line::from(vec![
            Span::styled("  Att. → : ", Style::default().fg(app.theme.transfer.get())),
            Span::styled(format!("{}_", s), Style::default().fg(app.theme.transfer.get())),
            mute_indicator,
        ]),
        TransferMode::AttendedPending => Line::from(vec![
            Span::styled(
                "  Attended: call ringing…",
                Style::default().fg(app.theme.attention.get()),
            ),
            mute_indicator,
        ]),
        TransferMode::None => {
            if app.in_active_call() {
                Line::from(vec![
                    Span::styled("  DTMF: ", Style::default().fg(app.theme.accent.get())),
                    Span::styled(
                        format!("{}_", app.dial.dtmf),
                        Style::default().fg(app.theme.accent.get()),
                    ),
                    mute_indicator,
                ])
            } else if app.dial.mode == InputMode::HistoryNav {
                Line::from(vec![
                    Span::styled("  Hist: ", Style::default().fg(app.theme.attention.get())),
                    Span::raw(format!("{}_", app.dial.input)),
                    mute_indicator,
                ])
            } else {
                // Render text split at cursor so the terminal cursor lands in the right spot
                let cursor = app.dial.cursor.min(app.dial.input.len());
                let before = &app.dial.input[..cursor];
                let after = &app.dial.input[cursor..];
                // prefix width: "  Dial: " = 8 chars
                let cursor_x = area.x + 8 + before.chars().count() as u16;
                f.set_cursor_position((cursor_x, area.y));
                Line::from(vec![
                    Span::styled("  Dial: ", Style::default().fg(app.theme.accent.get())),
                    Span::raw(before),
                    Span::raw(after),
                    mute_indicator,
                ])
            }
        }
    };
    f.render_widget(Paragraph::new(line), area);
}

fn render_keys(f: &mut Frame, app: &App, area: Rect) {
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
                String::new() // hints are in the call history title bar
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

fn render_log(f: &mut Frame, app: &App, area: Rect) {
    let visible = area.height.saturating_sub(1) as usize; // -1 for border
    let total = app.log.entries.len();
    // scroll=0 means bottom; scroll=N means N lines up from bottom
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

    let scroll_hint = if app.log.scroll > 0 {
        format!("Events ↑{} (↓ scroll down)", app.log.scroll)
    } else {
        "Events  (↑/↓ scroll)".to_string()
    };
    let list = List::new(items).block(Block::default().title(scroll_hint).borders(Borders::TOP));
    f.render_widget(list, area);
}

fn render_baresip_log(f: &mut Frame, app: &App, area: Rect) {
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

    let list = List::new(items).block(Block::default().title(title).borders(Borders::TOP));
    f.render_widget(list, area);
}

fn render_call_history(f: &mut Frame, app: &App, area: Rect) {
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
    let title: Line = if app.call_history.search_mode {
        Line::from(vec![
            Span::styled("Call History", accent),
            Span::raw(format!(
                "  / {}_  [Esc] cancel  [Enter] confirm",
                app.call_history.search_query
            )),
        ])
    } else if !app.call_history.search_query.is_empty() {
        Line::from(vec![
            Span::styled("Call History", accent),
            Span::raw(format!(
                "  /{} ({}/{})  [↑↓] nav  [Enter] redial  [d] del  [Esc] clear  [c] close",
                app.call_history.search_query, filtered_len, total
            )),
        ])
    } else if total == 0 {
        Line::from(vec![
            Span::styled("Call History", accent),
            Span::raw("  (empty)  [c/Esc] close"),
        ])
    } else {
        Line::from(vec![
            Span::styled("Call History", accent),
            Span::raw(format!(
                " ({}/{})  [d] del  [D] clear  [/] search  [c/Esc] close",
                if filtered_len > 0 { sel + 1 } else { 0 },
                filtered_len
            )),
        ])
    };

    f.render_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::TOP)),
        area,
    );
}

fn call_history_item<'a>(e: &'a CallHistoryEntry, app: &App) -> ListItem<'a> {
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
        Span::styled(format!("  {}", e.ts), Style::default().fg(app.theme.subtle.get())),
    ]);

    ListItem::new(line)
}

fn render_history_search(f: &mut Frame, app: &App, area: Rect) {
    let filtered = crate::history::fuzzy_filter(&app.dial.history, &app.dial.query);

    let max_visible: usize = 8;
    let visible = filtered.len().min(max_visible);
    let popup_h = (visible as u16 + 3)
        .max(4)
        .min(area.height.saturating_sub(2));
    let popup_w = area.width.saturating_sub(6).max(30);
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + area.height.saturating_sub(popup_h + 2);
    let popup_area = Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" History  (↑↓ navigate · Enter select · Esc cancel) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.accent.get()));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    f.render_widget(
        Paragraph::new(format!(" / {}_", app.dial.query))
            .style(Style::default().fg(app.theme.attention.get())),
        chunks[0],
    );

    // Scroll so selected item stays in view
    let scroll = app.dial.selected.saturating_sub(visible.saturating_sub(1));
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(i, entry)| {
            if i == app.dial.selected {
                ListItem::new(format!(" {}", entry)).style(
                    Style::default()
                        .fg(app.theme.attention.get())
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ListItem::new(format!(" {}", entry)).style(Style::default())
            }
        })
        .collect();

    f.render_widget(List::new(items), chunks[1]);
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

