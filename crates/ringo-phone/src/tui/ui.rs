use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Color,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Widget, Wrap,
    },
};

use crate::config::Theme;
use crate::mouse::{ClickAction, HitMap, ListId};

use super::app::InputMode;
use super::{App, RegStatus, TransferMode};

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    app.hits.clear();

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
    let cmd_h = if app.command.active || app.command.error.is_some() || app.command.notice.is_some()
    {
        1
    } else {
        hint_rows(&app.hints(), inner.width)
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

    // A click beside an overlay closes it. Pushed before the overlay so the
    // overlay's own regions win inside it; confirmations and the contact form
    // cover the whole screen with Inert and so are never dismissed this way.
    let overlay_open = app.log.show
        || app.help_show
        || app.details_show
        || app.call_history.show
        || app.contacts_state.show
        || app.dial.mode == InputMode::HistorySearch;
    if overlay_open {
        app.hits.push(area, ClickAction::Dismiss);
    }

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
        // No ↑↓ / PgUp/PgDn here: scrolling with the arrow keys needs no
        // telling. g/G stays — jumping to the ends is not guessable.
        let nav_footer = [
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
        let text_input = app.in_text_input();
        let content = render_modal(
            f,
            &app.theme,
            &mut app.hits,
            80,
            80,
            &title,
            footer,
            text_input,
            Some(ListId::Logs),
        );
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
    } else if app.details_show {
        render_details(f, app);
    } else if app.call_history.show {
        super::call_history::render(f, app, centered_rect(area, 80, 80));
        if let Some(kind) = app.call_history.delete_confirm {
            let q = match kind {
                super::app::HistoryDelete::One => "Delete the selected call?".to_string(),
                super::app::HistoryDelete::All => "Clear the entire call history?".to_string(),
            };
            render_confirm_popup(
                f,
                &app.theme,
                &mut app.hits,
                &q,
                "Delete",
                app.confirm_yes,
                true,
            );
        }
    } else if app.contacts_state.show {
        super::contacts::render(f, app, centered_rect(area, 80, 80));
        if let Some(ci) = app.contacts_state.delete_confirm {
            let name = app.contacts.get(ci).map(|c| c.name.as_str()).unwrap_or("?");
            let q = format!("Delete \"{}\"?", name);
            render_confirm_popup(
                f,
                &app.theme,
                &mut app.hits,
                &q,
                "Delete",
                app.confirm_yes,
                true,
            );
        }
    }

    if app.dial.mode == InputMode::HistorySearch {
        super::dial::render_history_search(f, app, inner);
    }

    // An open command bar: a click anywhere else leaves it, as Esc does, and is
    // not also a click on what lies there. The bar itself stays inert.
    if app.command.active {
        app.hits.push(area, ClickAction::Key(esc()));
        app.hits.push(chunks[5], ClickAction::Inert);
    }

    // Confirmation popups sit on top of everything.
    if app.quit_confirm {
        render_confirm_popup(
            f,
            &app.theme,
            &mut app.hits,
            "Quit ringo?",
            "Quit",
            app.confirm_yes,
            false,
        );
    } else if app.switch_confirm {
        render_confirm_popup(
            f,
            &app.theme,
            &mut app.hits,
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
pub(crate) fn render_scrollbar(
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
/// The frame is inert to clicks, so they do not reach what it covers; `scroll`
/// names the list the wheel moves inside it, if any.
#[allow(clippy::too_many_arguments)]
fn render_modal(
    f: &mut Frame,
    theme: &Theme,
    hits: &mut HitMap,
    w_pct: u16,
    h_pct: u16,
    title: &str,
    footer: &[Hint],
    text_input: bool,
    scroll: Option<ListId>,
) -> Rect {
    let rect = centered_rect(f.area(), w_pct, h_pct);
    f.render_widget(Clear, rect);
    hits.push(rect, ClickAction::Inert);
    if let Some(list) = scroll {
        hits.push(rect, ClickAction::Scroll { list });
    }
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
            let footer_area = Rect::new(inner.x, fy, inner.width, footer_h);
            render_hint_bar(f, footer_area, footer, theme);
            // `:` opens the command bar over an open overlay, so a plain-character
            // footer hint would be typed into it.
            record_hint_hits(hits, footer_area, footer, text_input);
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
    hits: &mut HitMap,
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
    // Modal: nothing outside the buttons is clickable while it is up.
    hits.push(area, ClickAction::Inert);
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
    let cancel_span = Span::styled("  Cancel  ", cancel);
    let confirm_span = Span::styled(format!("  {confirm_label}  "), confirm);
    let (cancel_w, confirm_w) = (cancel_span.width() as u16, confirm_span.width() as u16);
    let buttons = Line::from(vec![cancel_span, Span::raw("   "), confirm_span]);
    let row = Rect::new(inner.x, inner.y + 2, inner.width, 1);
    f.render_widget(Paragraph::new(buttons).alignment(Alignment::Center), row);

    // Same centring as the paragraph above. Esc and `y` are what every confirm
    // handler reads as cancel and confirm, whatever the button highlight says.
    let left = row.x + (row.width / 2).saturating_sub((cancel_w + 3 + confirm_w) / 2);
    hits.push(
        Rect::new(left, row.y, cancel_w, 1).intersection(row),
        ClickAction::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    );
    hits.push(
        Rect::new(left + cancel_w + 3, row.y, confirm_w, 1).intersection(row),
        ClickAction::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
    );
}

/// Static key/command reference shown in the Help modal.
fn render_help(f: &mut Frame, app: &mut App) {
    let content = render_modal(
        f,
        &app.theme,
        &mut app.hits,
        60,
        70,
        "Help",
        &[("Esc", "close")],
        false,
        None,
    );
    let accent = Style::default().fg(app.theme.accent.get());
    let subtle = Style::default().fg(app.theme.subtle.get());
    let width = content.width as usize;
    let mut lines = vec![Line::from(Span::styled("  Keys", subtle))];
    for (key, desc) in HELP_KEYS {
        let words: Vec<&str> = desc.split(' ').collect();
        let wrapped = wrap_units(&words, " ", width.saturating_sub(HELP_DESC_COL));
        for (i, part) in wrapped.into_iter().enumerate() {
            // Continuation lines hang under the description column.
            let mut spans = if i == 0 {
                vec![
                    Span::styled(format!("  {key:<10}"), accent),
                    Span::styled("→ ", subtle),
                ]
            } else {
                vec![Span::raw(" ".repeat(HELP_DESC_COL))]
            };
            spans.push(Span::raw(part));
            lines.push(Line::from(spans));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  Commands (:)", subtle)));
    for part in wrap_units(HELP_COMMANDS, "  ", width.saturating_sub(2)) {
        lines.push(Line::from(format!("  {part}")));
    }
    f.render_widget(Paragraph::new(lines), content);
}

/// Where a help description starts: two spaces, the 10-wide key, "→ ".
const HELP_DESC_COL: usize = 14;

const HELP_KEYS: &[(&str, &str)] = &[
    ("d", "dial"),
    ("a", "accept incoming"),
    ("s", "silence the ringtone"),
    ("b / Del", "hang up (reject if ringing)"),
    ("h", "hold / resume (toggle)"),
    ("m", "mute the microphone"),
    ("M", "deafen (microphone and speaker)"),
    ("t / T", "blind / attended transfer (t: deflect if ringing)"),
    ("Tab", "switch active call"),
    ("i", "call details"),
    ("l", "logs"),
    ("c", "call history"),
    ("f", "contacts"),
    ("?", "this help"),
    (":", "command"),
    ("q", "quit"),
];

/// Command-bar commands; wrapped between entries, never inside one.
const HELP_COMMANDS: &[&str] = &[
    "dial <n>",
    "hangup",
    "accept",
    "reject [code]",
    "hold",
    "resume",
    "mute",
    "deafen",
    "silence",
    "dtmf <digits>",
    "transfer <uri>",
    "deflect <uri>",
    "register",
    "unregister",
    "info",
    "log",
    "history",
    "contacts",
    "mouse",
    "edit",
    "switch",
    "quit",
];

/// Greedy wrap of `units` joined by `sep` into lines at most `width` columns
/// wide, breaking only between units. A unit wider than `width` gets a line of
/// its own and is cut by the widget.
fn wrap_units(units: &[&str], sep: &str, width: usize) -> Vec<String> {
    let cols = |s: &str| Span::raw(s).width();
    let width = width.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for unit in units.iter().filter(|u| !u.is_empty()) {
        if !cur.is_empty() && cols(&cur) + cols(sep) + cols(unit) > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push_str(sep);
        }
        cur.push_str(unit);
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Call-details overlay: peer + the configured inbound-header views.
fn render_details(f: &mut Frame, app: &mut App) {
    let content = render_modal(
        f,
        &app.theme,
        &mut app.hits,
        60,
        60,
        "Call details",
        &[("Esc", "close")],
        false,
        None,
    );
    let accent = Style::default().fg(app.theme.accent.get());
    let subtle = Style::default().fg(app.theme.subtle.get());
    let mut lines = Vec::new();
    if let Some(call) = app.calls.get(app.selected_call) {
        lines.push(Line::from(vec![
            Span::styled("  Peer  ", subtle),
            Span::styled(call.peer.clone(), Style::default()),
        ]));
        if let Some(dn) = &call.peer_display_name {
            lines.push(Line::from(vec![
                Span::styled("  Name  ", subtle),
                Span::styled(dn.clone(), Style::default()),
            ]));
        }
        lines.push(Line::from(""));
        if call.header_views.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No header views for this call.",
                subtle,
            )));
        } else {
            for (label, value) in &call.header_views {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {label}  "), accent),
                    Span::styled(value.clone(), Style::default()),
                ]));
            }
        }
    } else {
        lines.push(Line::from(Span::styled("  No active call.", subtle)));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), content);
}

// ─── Status Bar ──────────────────────────────────────────────────────────────

fn render_status_bar(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
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
        // Attention, not danger: nothing went wrong, but calls will not arrive
        // and that has to stay visible until it is undone.
        RegStatus::Unregistered => (
            "○ Unregistered",
            Style::default().fg(app.theme.attention.get()),
        ),
        RegStatus::Registering => (
            "◌ Registering",
            Style::default().fg(app.theme.attention.get()),
        ),
        RegStatus::Unknown => (
            "○ Connecting",
            Style::default().fg(app.theme.attention.get()),
        ),
    };
    let reg_x = area.x + spans.iter().map(|s| s.width() as u16).sum::<u16>();
    let reg_span = Span::styled(reg_text, reg_style);
    let reg_w = reg_span.width() as u16;
    spans.push(reg_span);

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

    // Muted / deafened. Deafened implies muted, so show the stronger one alone
    // rather than two badges saying overlapping things.
    if app.deafened || app.muted {
        spans.push(sep.clone());
        spans.push(Span::styled(
            if app.deafened { "DEAFENED" } else { "MUTED" },
            Style::default()
                .fg(app.theme.danger.get())
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Silenced ringtone. Silencing produces no sound and no state change the
    // user can otherwise see, so this line is the only confirmation it worked.
    if app.ring_silenced && app.has_incoming_ringing() {
        spans.push(sep.clone());
        spans.push(Span::styled(
            "RING SILENCED",
            Style::default().fg(app.theme.subtle.get()),
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

    // Starting a registration that is already running would only repeat it.
    if !app.overlay_open()
        && matches!(
            app.reg_status,
            RegStatus::Failed(_) | RegStatus::Unregistered
        )
    {
        app.hits.push(
            Rect::new(reg_x, area.y, reg_w, 1).intersection(bar_chunks[0]),
            ClickAction::Command("register"),
        );
    }
}

// ─── Command / Hint Bar ──────────────────────────────────────────────────────

fn esc() -> KeyEvent {
    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
}

fn render_command_bar(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
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
    } else if let Some(notice) = &app.command.notice {
        f.render_widget(
            Paragraph::new(format!(" {notice}")).style(Style::default().fg(app.theme.subtle.get())),
            area,
        );
    } else {
        render_hints(f, app, area);
    }
}

impl App {
    /// Whether an overlay or popup covers the phone screen, whose own regions
    /// are then not clickable.
    pub(super) fn overlay_open(&self) -> bool {
        self.log.show
            || self.help_show
            || self.details_show
            || self.call_history.show
            || self.contacts_state.show
            || self.dial.mode == InputMode::HistorySearch
            || self.quit_confirm
            || self.switch_confirm
    }

    /// The base hint line's keys for the current mode. Overlay-specific hints
    /// live in each modal's own footer; this is the always-visible call-action
    /// bar.
    ///
    /// A method rather than a free function so the TUI tests can reach it
    /// through `App`, which the crate already exports. What the bar offers in a
    /// given state is behaviour worth asserting on; the rendered pixels are
    /// ratatui's business.
    pub fn hints(&self) -> Vec<(&'static str, &'static str)> {
        let app = self;
        match &app.transfer_mode {
            TransferMode::BlindInput(_)
            | TransferMode::AttendedInput(_)
            | TransferMode::DeflectInput { .. } => {
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
                // A call in progress gets only what you reach for while talking.
                // Logs, history, contacts and quit stay available — they are just
                // not what the bar is for at that moment, and listing them crowds
                // out the four keys that matter. `?` reads "more" rather than
                // "help" so it is visible that something was set aside.
                InputMode::Normal if app.has_any_call() => {
                    let mut h: Vec<Hint> = Vec::new();
                    if app.has_incoming_ringing() {
                        h.push(("a", "accept"));
                        if !app.ring_silenced {
                            h.push(("s", "silence"));
                        }
                    }
                    if app.selected_call_is_ringing_incoming() {
                        h.push(("b", "reject"));
                        h.push(("t", "deflect"));
                    } else {
                        h.push(("b", "hangup"));
                    }
                    // One key, and the label says which way it will go —
                    // clearer than "h/r hold/resume", and shorter.
                    if app.selected_call_on_hold() {
                        h.push(("h", "resume"));
                    } else if app.in_active_call() {
                        h.push(("h", "hold"));
                    }
                    if app.in_active_call() {
                        h.push(("m/M", "mute/deafen"));
                        h.push(("t/T", "xfer"));
                    }
                    if app.calls.len() > 1 {
                        h.push(("Tab", "switch"));
                    }
                    h.push(("?", "more"));
                    h
                }
                InputMode::Normal => vec![
                    ("d", "dial"),
                    ("l", "logs"),
                    ("c", "history"),
                    ("f", "contacts"),
                    ("?", "help"),
                    (":", "cmd"),
                    ("q", "quit"),
                ],
            },
        }
    }
}

fn render_hints(f: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let hints = app.hints();
    render_hint_bar(f, area, &hints, &app.theme);
    // Under an overlay or popup the keys go to that, not to the bar's meaning.
    if !app.overlay_open() {
        let text_input = app.in_text_input();
        record_hint_hits(&mut app.hits, area, &hints, text_input);
    }
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
    hint_line(hints, |_| key_style, |_| lbl_style, lbl_style)
}

/// The hint line with a style per hint, so the same text can be drawn for real
/// ([`styled_hints`]) and for measuring ([`hint_spans`]).
fn hint_line(
    hints: &[Hint],
    key_style: impl Fn(usize) -> Style,
    label_style: impl Fn(usize) -> Style,
    separator_style: Style,
) -> Line<'static> {
    let mut spans: Vec<Span> = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", separator_style));
        }
        let key = match key.strip_prefix('^') {
            Some(c) => format!("C-{}", c.to_lowercase()),
            None => (*key).to_string(),
        };
        spans.push(Span::styled(key, key_style(i)));
        if !label.is_empty() {
            spans.push(Span::styled(format!(" {label}"), label_style(i)));
        }
    }
    Line::from(spans)
}

/// Where each hint lands when the line wraps in a `width`×`height` area, as
/// (hint index, row, first col, end col) — one entry per row a hint touches.
/// Measured by drawing the line through the same `Paragraph` as
/// [`render_hint_bar`], so it cannot drift from ratatui's word wrapping.
fn hint_spans(hints: &[Hint], width: u16, height: u16) -> Vec<(usize, u16, u16, u16)> {
    let area = Rect::new(0, 0, width, height);
    if area.is_empty() {
        return Vec::new();
    }
    // Each hint gets its own indexed colour; the separators keep the default.
    let marker = |i: usize| Style::default().fg(Color::Indexed(i as u8));
    let hints = &hints[..hints.len().min(usize::from(u8::MAX) + 1)];
    let line = hint_line(hints, marker, marker, Style::default());
    let mut buf = Buffer::empty(area);
    Paragraph::new(line)
        .wrap(Wrap { trim: false })
        .render(area, &mut buf);

    let mut spans: Vec<(usize, u16, u16, u16)> = Vec::new();
    for row in 0..height {
        for col in 0..width {
            let cell = &buf[(col, row)];
            let Color::Indexed(i) = cell.fg else { continue };
            if cell.symbol() == " " {
                continue;
            }
            match spans.last_mut() {
                Some(last) if last.0 == usize::from(i) && last.1 == row => last.3 = col + 1,
                _ => spans.push((usize::from(i), row, col, col + 1)),
            }
        }
    }
    spans
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

/// The key a click on the hint `key` stands for. Alternatives (`m/M`, `a/e/d`,
/// `↑/↓`) give their first; `^X` is Ctrl+x, `⇧↑` Shift+Up; unknown names give
/// `None`.
pub(crate) fn key_for_hint(key: &str) -> Option<KeyEvent> {
    let first = if key == "/" {
        key
    } else {
        key.split('/').next()?
    };
    let plain = |code| Some(KeyEvent::new(code, KeyModifiers::NONE));
    match first {
        "Enter" => return plain(KeyCode::Enter),
        "Esc" => return plain(KeyCode::Esc),
        "Tab" | "⇥" => return plain(KeyCode::Tab),
        "↑" => return plain(KeyCode::Up),
        "↓" => return plain(KeyCode::Down),
        // Only on its own: the form's `Space/←→` changes the focused field,
        // which a click on the field itself already does.
        "Space" if key == "Space" => return plain(KeyCode::Char(' ')),
        "Space" => return None,
        "⇧↑" => return Some(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT)),
        "⇧↓" => return Some(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT)),
        _ => {}
    }
    if let Some(ctrl) = first.strip_prefix('^') {
        let mut chars = ctrl.chars();
        return match (chars.next(), chars.next()) {
            (Some(c), None) => Some(KeyEvent::new(
                KeyCode::Char(c.to_ascii_lowercase()),
                KeyModifiers::CONTROL,
            )),
            _ => None,
        };
    }
    let mut chars = first.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => {
            let modifiers = if c.is_uppercase() {
                KeyModifiers::SHIFT
            } else {
                KeyModifiers::NONE
            };
            Some(KeyEvent::new(KeyCode::Char(c), modifiers))
        }
        _ => None,
    }
}

/// Make each hint of a bar rendered by [`render_hint_bar`] into `area` clickable.
/// While a text input is active, a plain character key would be typed into it
/// instead of acting, so only Enter/Esc/Tab/arrows/Ctrl chords stay clickable.
pub(crate) fn record_hint_hits(hits: &mut HitMap, area: Rect, hints: &[Hint], text_input: bool) {
    let bar = Rect::new(
        area.x + HINT_INDENT,
        area.y,
        area.width.saturating_sub(HINT_INDENT),
        area.height,
    );
    for (hint, row, start, end) in hint_spans(hints, bar.width, bar.height) {
        let Some(key) = key_for_hint(hints[hint].0) else {
            continue;
        };
        let typed =
            matches!(key.code, KeyCode::Char(_)) && !key.modifiers.contains(KeyModifiers::CONTROL);
        if text_input && typed {
            continue;
        }
        hits.push(
            Rect::new(bar.x + start, bar.y + row, end - start, 1),
            ClickAction::Key(key),
        );
    }
}

/// Record a list overlay: inert frame, wheel over the whole frame, and a row
/// region per visible row. `offset` is the index of the first row shown in
/// `list_area`, so the regions stay right in a scrolled list.
pub(crate) fn record_list_hits(
    hits: &mut HitMap,
    list: ListId,
    frame: Rect,
    list_area: Rect,
    offset: usize,
    total: usize,
) {
    hits.push(frame, ClickAction::Inert);
    hits.push(frame, ClickAction::Scroll { list });
    for (i, index) in (offset..total).take(list_area.height as usize).enumerate() {
        hits.push(
            Rect::new(list_area.x, list_area.y + i as u16, list_area.width, 1),
            ClickAction::Row { list, index },
        );
    }
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
    use super::{
        HELP_COMMANDS, Hint, key_for_hint, record_hint_hits, render_hint_bar, styled_hints,
        wrap_units,
    };

    #[test]
    fn help_text_wraps_between_units_instead_of_being_cut() {
        let lines = wrap_units(HELP_COMMANDS, "  ", 30);
        assert!(lines.len() > 3, "{lines:?}");
        assert!(lines.iter().all(|l| l.chars().count() <= 30), "{lines:?}");
        // Nothing is lost, and "dial <n>" is never split.
        assert_eq!(lines.join("  "), HELP_COMMANDS.join("  "));
        assert!(lines.iter().any(|l| l.contains("dial <n>")));

        let words = ["blind", "/", "attended", "transfer"];
        assert_eq!(
            wrap_units(&words, " ", 14),
            vec!["blind /", "attended", "transfer"]
        );
        // A unit wider than the line stays whole on a line of its own.
        assert_eq!(wrap_units(&["verylongword"], " ", 4), vec!["verylongword"]);
    }
    use crate::config::Theme;
    use crate::mouse::{ClickAction, HitMap};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    fn plain(code: KeyCode) -> Option<KeyEvent> {
        Some(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn key_for_hint_maps_names_alternatives_and_chords() {
        assert_eq!(key_for_hint("b"), plain(KeyCode::Char('b')));
        assert_eq!(key_for_hint("Tab"), plain(KeyCode::Tab));
        assert_eq!(key_for_hint("Esc"), plain(KeyCode::Esc));
        assert_eq!(key_for_hint("Enter"), plain(KeyCode::Enter));
        assert_eq!(key_for_hint("?"), plain(KeyCode::Char('?')));
        assert_eq!(key_for_hint("/"), plain(KeyCode::Char('/')));
        assert_eq!(key_for_hint("m/M"), plain(KeyCode::Char('m')));
        assert_eq!(key_for_hint("t/T"), plain(KeyCode::Char('t')));
        assert_eq!(key_for_hint("g/G"), plain(KeyCode::Char('g')));
        assert_eq!(key_for_hint("↑/↓"), plain(KeyCode::Up));
        assert_eq!(key_for_hint("Space"), plain(KeyCode::Char(' ')));
        assert_eq!(
            key_for_hint("⇧↑/⇧↓"),
            Some(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT))
        );
        assert_eq!(
            key_for_hint("^R"),
            Some(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL))
        );
        assert_eq!(
            key_for_hint("X"),
            Some(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::SHIFT))
        );
        assert_eq!(key_for_hint("Space/←→"), None);
        assert_eq!(key_for_hint("⇥"), plain(KeyCode::Tab));
        assert_eq!(key_for_hint(""), None);
    }

    /// The text under every recorded hint region, read back from the rendered
    /// frame — what a click on it would hit.
    fn region_texts(hints: &[Hint], width: u16, text_input: bool) -> Vec<String> {
        let theme = Theme::default();
        let area = Rect::new(0, 0, width, 8);
        let mut terminal = Terminal::new(TestBackend::new(width, 8)).unwrap();
        terminal
            .draw(|f| render_hint_bar(f, area, hints, &theme))
            .unwrap();
        let mut hits = HitMap::default();
        record_hint_hits(&mut hits, area, hints, text_input);
        let buf = terminal.backend().buffer();
        let mut out = Vec::new();
        for row in 0..8 {
            for col in 0..width {
                if let Some(ClickAction::Key(key)) = hits.hit(col, row) {
                    let _ = key;
                    out.push((col, row));
                }
            }
        }
        // Group the hit cells per row run into strings.
        let mut texts: Vec<String> = Vec::new();
        let mut prev: Option<(u16, u16)> = None;
        for (col, row) in out {
            let symbol = buf[(col, row)].symbol().to_string();
            match prev {
                Some((pc, pr)) if pr == row && pc + 1 == col => {
                    texts.last_mut().unwrap().push_str(&symbol)
                }
                _ => texts.push(symbol),
            }
            prev = Some((col, row));
        }
        texts
    }

    #[test]
    fn hint_regions_cover_the_key_and_label_on_one_row() {
        let hints = [("d", "dial"), ("^R", "search"), ("Esc", "")];
        assert_eq!(
            region_texts(&hints, 60, false),
            vec!["d dial", "C-r search", "Esc"]
        );
    }

    #[test]
    fn hint_regions_follow_the_wrapping_on_a_narrow_bar() {
        let hints = [
            ("a", "accept"),
            ("b", "reject"),
            ("t", "deflect"),
            ("Tab", "switch"),
        ];
        let texts = region_texts(&hints, 18, false);
        // Every word is still covered, wherever it wrapped to.
        let joined = texts.join("|");
        for word in [
            "a", "accept", "b", "reject", "t", "deflect", "Tab", "switch",
        ] {
            assert!(joined.contains(word), "{word} missing in {joined}");
        }
        assert!(
            texts.len() > hints.len(),
            "a hint split across rows has a region per row"
        );
    }

    #[test]
    fn text_input_leaves_only_non_character_hints_clickable() {
        let hints = [
            ("d", "dial"),
            ("Enter", "go"),
            ("^R", "search"),
            ("?", "more"),
        ];
        assert_eq!(
            region_texts(&hints, 60, true),
            vec!["Enter go", "C-r search"]
        );
    }

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
