use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
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
    let outer = Block::default()
        .title(title_left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // The hint bar wraps onto extra rows on narrow terminals; size it to fit so
    // it doesn't overflow. The command bar / error line is always one row.
    let cmd_h = if app.command.active || app.command.error.is_some() {
        1
    } else {
        hint_rows(&normal_hints(app), inner.width)
    };

    // Fixed layout; all secondary views (Logs, Help, Call history, Contacts) are
    // centered modal overlays drawn on top.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),        // [0] calls
            Constraint::Length(1),     // [1] spacer
            Constraint::Length(1),     // [2] dial
            Constraint::Length(1),     // [3] error reason
            Constraint::Length(1),     // [4] status bar
            Constraint::Length(cmd_h), // [5] hints / command bar
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
        // Tail-follow / wrap status shown in the title.
        let mut status = if app.log.scroll == 0 {
            "  ● live".to_string()
        } else {
            format!("  ↑{} paused", app.log.scroll)
        };
        if app.log.wrap {
            status.push_str("  wrap");
        }
        let title = if app.log.search_mode {
            format!("Logs  / {}_", app.log.search_query)
        } else if !app.log.search_query.is_empty() {
            format!(
                "Logs  /{}  ({}){}",
                app.log.search_query,
                app.log_filtered().len(),
                status
            )
        } else {
            format!("Logs{status}")
        };
        let search_footer = [("Enter", "confirm"), ("Esc", "clear")];
        let nav_footer = [
            ("↑↓", "scroll"),
            ("PgUp/PgDn", "page"),
            ("g/G", "ends"),
            ("/", "search"),
            ("w", "wrap"),
            ("Esc", "close"),
        ];
        let footer: &[Hint] = if app.log.search_mode {
            &search_footer
        } else {
            &nav_footer
        };
        let content = render_modal(f, &app.theme, 80, 80, &title, footer);
        super::log::render_logs(f, app, content);
        let top = app
            .log
            .content_rows
            .saturating_sub(app.log.visible_height)
            .saturating_sub(app.log.scroll);
        render_scrollbar(
            f,
            &app.theme,
            centered_rect(area, 80, 80),
            app.log.content_rows,
            app.log.visible_height,
            top,
        );
    } else if app.help_show {
        render_help(f, app);
    } else if app.call_history.show {
        super::call_history::render(f, app, centered_rect(area, 80, 80));
        if let Some(kind) = app.call_history.delete_confirm {
            let q = match kind {
                super::app::HistoryDelete::One => "Delete the selected call?".to_string(),
                super::app::HistoryDelete::All => "Clear the entire call history?".to_string(),
            };
            render_confirm_popup(f, &app.theme, &q, "Delete", app.confirm_yes, true);
        }
    } else if app.contacts_state.show {
        super::contacts::render(f, app, centered_rect(area, 80, 80));
        if let Some(ci) = app.contacts_state.delete_confirm {
            let name = app.contacts.get(ci).map(|c| c.name.as_str()).unwrap_or("?");
            let q = format!("Delete \"{}\"?", name);
            render_confirm_popup(f, &app.theme, &q, "Delete", app.confirm_yes, true);
        }
    }

    if app.dial.mode == InputMode::HistorySearch {
        super::dial::render_history_search(f, app, inner);
    }

    // Confirmation popups sit on top of everything.
    if app.quit_confirm {
        render_confirm_popup(f, &app.theme, "Quit ringo?", "Quit", app.confirm_yes, false);
    } else if app.switch_confirm {
        render_confirm_popup(
            f,
            &app.theme,
            "Switch profile?",
            "Switch",
            app.confirm_yes,
            false,
        );
    }
}

/// Draw a vertical scrollbar on the right border of a bordered `rect` (between
/// its top and bottom borders). No-op when everything fits. `total` is the item
/// / row count, `visible` the viewport size, `position` the top index shown.
pub(super) fn render_scrollbar(
    f: &mut Frame,
    theme: &Theme,
    rect: Rect,
    total: usize,
    visible: usize,
    position: usize,
) {
    if total <= visible || rect.height < 3 {
        return;
    }
    // Map the scroll range (0..=total-visible) onto the track so the thumb sits
    // at the bottom when the view is at the end.
    let range = total - visible;
    let mut state = ScrollbarState::new(range).position(position.min(range));
    let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .thumb_style(Style::default().fg(theme.accent.get()))
        .track_style(Style::default().fg(theme.subtle.get()));
    f.render_stateful_widget(bar, rect.inner(Margin::new(0, 1)), &mut state);
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
    footer: &[Hint],
) -> Rect {
    let rect = centered_rect(f.area(), w_pct, h_pct);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            title.to_string(),
            Style::default().fg(theme.accent.get()),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    if !footer.is_empty() {
        let footer_h = hint_rows(footer, inner.width);
        if inner.height > footer_h {
            let fy = inner.y + inner.height - footer_h;
            render_hint_bar(
                f,
                Rect::new(inner.x, fy, inner.width, footer_h),
                footer,
                theme,
            );
            return Rect::new(inner.x, inner.y, inner.width, inner.height - footer_h);
        }
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
    danger: bool,
) {
    // Destructive actions (delete) use the danger colour; benign ones (quit,
    // switch) use the accent colour.
    let accent = if danger {
        theme.danger.get()
    } else {
        theme.accent.get()
    };
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
        .border_style(Style::default().fg(accent))
        .title(Span::styled("Confirm", Style::default().fg(accent)))
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

    let cancel = if !yes {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtle.get())
    };
    let confirm = if yes {
        Style::default()
            .fg(accent)
            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtle.get())
    };
    let buttons = Line::from(vec![
        Span::styled("  Cancel  ", cancel),
        Span::raw("   "),
        Span::styled(format!("  {confirm_label}  "), confirm),
    ]);
    f.render_widget(
        Paragraph::new(buttons).alignment(Alignment::Center),
        Rect::new(inner.x, inner.y + 2, inner.width, 1),
    );
}

/// Static key/command reference shown in the Help modal.
fn render_help(f: &mut Frame, app: &App) {
    let content = render_modal(f, &app.theme, 60, 70, "Help", &[("Esc", "close")]);
    let accent = Style::default().fg(app.theme.accent.get());
    let subtle = Style::default().fg(app.theme.subtle.get());
    let row = |key: &str, desc: &str| {
        Line::from(vec![
            Span::styled(format!("  {key:<10}"), accent),
            Span::styled("→ ", subtle),
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
    // Call deflected (302)
    if let Some(d) = &app.deflected {
        spans.push(sep.clone());
        spans.push(Span::styled(
            format!(
                "↪ Deflected: {} → {}",
                d.display_name
                    .as_deref()
                    .map(|n| format!("{n} ({})", d.from))
                    .unwrap_or_else(|| d.from.clone()),
                d.target
            ),
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

/// The base hint line's keys for the current mode. Overlay-specific hints live
/// in each modal's own footer; this is the always-visible call-action bar.
fn normal_hints(app: &App) -> Vec<Hint<'static>> {
    match &app.transfer_mode {
        TransferMode::BlindInput(_) | TransferMode::AttendedInput(_) => {
            vec![("Enter", "send"), ("Tab", "contacts"), ("Esc", "cancel")]
        }
        TransferMode::AttendedPending => vec![("X", "execute transfer"), ("Esc", "abort")],
        TransferMode::None => match app.dial.mode {
            InputMode::Dial | InputMode::HistoryNav => vec![
                ("Enter", "dial"),
                ("Tab", "contacts"),
                ("Esc", "cancel"),
                ("↑/↓", "history"),
                ("^R", "search"),
            ],
            InputMode::HistorySearch => Vec::new(),
            InputMode::Normal => {
                let mut h: Vec<Hint> = vec![("d", "dial")];
                if app.has_incoming_ringing() {
                    h.push(("a", "accept"));
                }
                if app.has_any_call() {
                    h.push(("b", "hangup"));
                }
                if app.in_active_call() {
                    h.push(("h", "hold"));
                }
                if app.selected_call_on_hold() {
                    h.push(("r", "resume"));
                }
                if app.in_active_call() {
                    h.push(("m", "mute"));
                    h.push(("t", "xfer"));
                    h.push(("T", "att.xfer"));
                }
                if app.calls.len() > 1 {
                    h.push(("Tab", "switch"));
                }
                h.extend([
                    ("l", "logs"),
                    ("c", "history"),
                    ("f", "contacts"),
                    ("?", "help"),
                    (":", "cmd"),
                    ("q", "quit"),
                ]);
                h
            }
        },
    }
}

fn render_hints(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    render_hint_bar(f, area, &normal_hints(app), &app.theme);
}

/// One keybind hint: the key (or chord) and what it does. Structured so the
/// keys can later come from a configurable keymap instead of literals.
pub(crate) type Hint<'a> = (&'a str, &'a str);

/// Left indent shared by every hint bar so wrapped rows line up with the first.
const HINT_INDENT: u16 = 1;

/// Render `(key, label)` hints as styled spans — key in bold accent, label
/// subtle — for a which-key-ish look. Ctrl chords written `^X` display as `C-x`.
/// No leading indent (that's applied by [`render_hint_bar`] so wrapped rows line
/// up too); render with wrapping.
pub(crate) fn styled_hints(hints: &[Hint], theme: &Theme) -> Line<'static> {
    let key_style = Style::default()
        .fg(theme.accent.get())
        .add_modifier(Modifier::BOLD);
    let lbl_style = Style::default().fg(theme.subtle.get());
    let mut spans: Vec<Span> = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", lbl_style));
        }
        let key = match key.strip_prefix('^') {
            Some(c) => format!("C-{}", c.to_lowercase()),
            None => (*key).to_string(),
        };
        spans.push(Span::styled(key, key_style));
        if !label.is_empty() {
            spans.push(Span::styled(format!(" {label}"), lbl_style));
        }
    }
    Line::from(spans)
}

/// How many rows the hints need in an area of `width`, accounting for the shared
/// left indent, so a hint bar can wrap onto extra lines instead of overflowing.
pub(crate) fn hint_rows(hints: &[Hint], width: u16) -> u16 {
    let mut w = 0usize;
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            w += 3; // separator
        }
        // `^X` renders as `C-x` (3 cols).
        w += if key.starts_with('^') {
            key.chars().count() + 1
        } else {
            key.chars().count()
        };
        if !label.is_empty() {
            w += 1 + label.chars().count();
        }
    }
    let avail = width.saturating_sub(HINT_INDENT).max(1);
    (w as u16).div_ceil(avail).max(1)
}

/// Render a wrapping hint bar into `area`, indented so every (wrapped) row lines
/// up. Pair with [`hint_rows`] to size the area's height.
pub(crate) fn render_hint_bar(f: &mut Frame, area: Rect, hints: &[Hint], theme: &Theme) {
    let inner = Rect::new(
        area.x + HINT_INDENT,
        area.y,
        area.width.saturating_sub(HINT_INDENT),
        area.height,
    );
    f.render_widget(
        Paragraph::new(styled_hints(hints, theme)).wrap(Wrap { trim: false }),
        inner,
    );
}

#[cfg(test)]
mod tests {
    use super::styled_hints;
    use crate::config::Theme;

    #[test]
    fn hints_wrap_onto_more_rows_when_narrow() {
        use super::hint_rows;
        let theme = Theme::default();
        let hints = [
            ("d", "dial"),
            ("a", "accept"),
            ("b", "hangup"),
            ("q", "quit"),
        ];

        // Wide: fits on one row.
        assert_eq!(hint_rows(&hints, 200), 1);
        // Narrow: wraps onto more rows.
        assert!(hint_rows(&hints, 12) > 1);

        // The full line keeps every token (no truncation) — wrapping shows them.
        let s: String = styled_hints(&hints, &theme)
            .spans
            .iter()
            .map(|sp| sp.content.as_ref())
            .collect();
        assert!(s.contains("quit"));
    }
}
