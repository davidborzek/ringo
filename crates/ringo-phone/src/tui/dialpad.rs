//! The mouse dial pad: a keypad overlay opened by clicking the dial / DTMF line.
//! Every button is a key press, so digits go where typed digits go — into the
//! dial input, or out as DTMF during a call.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use super::app::{App, InputMode};
use crate::mouse::ClickAction;

const DIGITS: [[char; 3]; 4] = [
    ['1', '2', '3'],
    ['4', '5', '6'],
    ['7', '8', '9'],
    ['*', '0', '#'],
];

/// Columns per button, `[ x ]` plus a gap.
const BUTTON_W: u16 = 6;
const BUTTON_GAP: u16 = 1;

fn plain(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

impl App {
    /// The pad acts on the call in progress (DTMF) rather than on the dial input.
    pub(super) fn dialpad_sends_dtmf(&self) -> bool {
        self.dial.mode == InputMode::Normal && self.in_active_call()
    }

    /// A click on the dial line: open the pad, and the dial input with it when
    /// neither a call nor a dial is in progress.
    pub(super) fn open_dialpad(&mut self) {
        if !self.dialpad_sends_dtmf() && self.dial.mode == InputMode::Normal {
            self.dial.mode = InputMode::Dial;
            self.command.error = None;
        }
        self.dialpad_show = true;
    }

    /// Keep the pad only while there is something for it to act on.
    pub(super) fn sync_dialpad(&mut self) {
        let dialing = matches!(self.dial.mode, InputMode::Dial | InputMode::HistoryNav);
        if self.dialpad_show && !(dialing || self.dialpad_sends_dtmf()) {
            self.dialpad_show = false;
        }
    }
}

/// Make the dial line open the pad. `line` is the row `render_dial` drew into.
pub(super) fn record_dial_line(app: &mut App, line: Rect) {
    if !app.mouse_enabled || app.dialpad_show || app.overlay_open() {
        return;
    }
    let idle = app.dial.mode == InputMode::Normal && !app.in_active_call();
    let dialing = matches!(app.dial.mode, InputMode::Dial | InputMode::HistoryNav);
    if (idle && app.transfer_mode == super::app::TransferMode::None)
        || dialing
        || app.dialpad_sends_dtmf()
    {
        app.hits.push(line, ClickAction::OpenDialpad);
    }
}

/// Draw the pad centred in `area` and record its buttons; a click anywhere else
/// on `screen` closes it.
pub(super) fn render(f: &mut Frame, app: &mut App, screen: Rect, area: Rect) {
    let dtmf = app.dialpad_sends_dtmf();
    let rows: u16 = if dtmf { 4 } else { 5 };
    // Display line, a gap, the button rows, borders.
    let h = (rows + 4).min(area.height);
    let w = (3 * BUTTON_W + 2 * BUTTON_GAP + 4).min(area.width);
    let rect = Rect::new(
        area.x + area.width.saturating_sub(w) / 2,
        area.y + area.height.saturating_sub(h) / 2,
        w,
        h,
    );
    f.render_widget(Clear, rect);
    app.hits.push(screen, ClickAction::Dismiss);
    app.hits.push(rect, ClickAction::Inert);

    let accent = app.theme.accent.get();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            if dtmf { " DTMF " } else { " Dial " },
            Style::default().fg(accent),
        ))
        .title_alignment(Alignment::Center);
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    if inner.height == 0 {
        return;
    }

    let shown = if dtmf {
        app.dial.dtmf.clone()
    } else {
        format!("{}_", app.dial.input)
    };
    // The tail is what was just entered, so a long number keeps it in view.
    let width = inner.width.saturating_sub(2) as usize;
    let skip = shown.chars().count().saturating_sub(width);
    let shown: String = shown.chars().skip(skip).collect();
    f.render_widget(
        Paragraph::new(format!(" {shown}")).style(Style::default().fg(accent)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let mut buttons: Vec<Vec<(String, KeyEvent)>> = DIGITS
        .iter()
        .map(|row| {
            row.iter()
                .map(|&c| (c.to_string(), plain(KeyCode::Char(c))))
                .collect()
        })
        .collect();
    if !dtmf {
        buttons.push(vec![
            ("+".into(), plain(KeyCode::Char('+'))),
            ("⌫".into(), plain(KeyCode::Backspace)),
            ("Call".into(), plain(KeyCode::Enter)),
        ]);
    }

    let x0 = inner.x + inner.width.saturating_sub(3 * BUTTON_W + 2 * BUTTON_GAP) / 2;
    let style = Style::default().add_modifier(Modifier::REVERSED);
    for (r, row) in buttons.into_iter().enumerate() {
        let y = inner.y + 2 + r as u16;
        if y >= inner.y + inner.height {
            break;
        }
        for (c, (label, key)) in row.into_iter().enumerate() {
            let x = x0 + c as u16 * (BUTTON_W + BUTTON_GAP);
            let cell = Rect::new(x, y, BUTTON_W.min(inner.x + inner.width - x), 1);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(label, style)))
                    .alignment(Alignment::Center)
                    .style(style),
                cell,
            );
            app.hits.push(cell, ClickAction::Key(key));
        }
    }
}
