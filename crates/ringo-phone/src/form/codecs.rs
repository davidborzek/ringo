use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};
use ringo_core::event::CodecInfo;

use super::Term;
use crate::config::Theme;

/// One picker row: the baresip codec spec to store, a display label, and whether
/// it's selected.
struct Entry {
    spec: String,
    label: String,
    on: bool,
}

/// baresip's `audio_codecs` lookup defaults a bare name to 8000 Hz, so a codec at
/// another rate (opus 48k, G722 16k) must carry `name/srate/ch` or it's "not
/// found". We always emit the full spec from the codec's real rate/channels.
fn spec(c: &CodecInfo) -> String {
    format!("{}/{}/{}", c.name, c.srate, c.ch)
}

fn label(c: &CodecInfo) -> String {
    format!("{} ({} kHz, {} ch)", c.name, c.srate / 1000, c.ch)
}

/// Fallback codec set when baresip isn't up yet (profile edited without a live
/// session) — the codecs ringo always compiles in, with their real rate/channels.
fn fallback_codecs() -> Vec<CodecInfo> {
    [
        ("opus", 48000, 2),
        ("G722", 16000, 1),
        ("PCMU", 8000, 1),
        ("PCMA", 8000, 1),
    ]
    .into_iter()
    .map(|(name, srate, ch)| CodecInfo {
        name: name.into(),
        srate,
        ch,
    })
    .collect()
}

/// Edit a profile's ordered audio-codec preference: toggle codecs on/off and
/// reorder them (most-preferred first). `codecs` (baresip `name/srate/ch` specs)
/// is replaced in place on return; empty = baresip's default set/order.
pub(crate) fn run_codecs_submenu(
    terminal: &mut Term,
    codecs: &mut Vec<String>,
    theme: &Theme,
) -> Result<()> {
    // Offer the codecs baresip actually registered (this build); fall back to the
    // always-compiled set if no session has started the backend yet.
    let available = {
        let q = ringo_core::available_audio_codecs();
        if q.is_empty() { fallback_codecs() } else { q }
    };

    // Selected codecs first (in the profile's saved order, normalised to the full
    // spec so a previously bare/incorrect entry is fixed on save), then the rest.
    let mut entries: Vec<Entry> = Vec::new();
    for saved in codecs.iter() {
        let name = saved.split('/').next().unwrap_or(saved);
        match available.iter().find(|c| c.name.eq_ignore_ascii_case(name)) {
            Some(c) => entries.push(Entry {
                spec: spec(c),
                label: label(c),
                on: true,
            }),
            None => entries.push(Entry {
                spec: saved.clone(),
                label: saved.clone(),
                on: true,
            }),
        }
    }
    for c in &available {
        let name = c.name.as_str();
        if !entries
            .iter()
            .any(|e| e.spec.split('/').next() == Some(name))
        {
            entries.push(Entry {
                spec: spec(c),
                label: label(c),
                on: false,
            });
        }
    }
    let mut focused: usize = 0;

    loop {
        let n = entries.len();
        terminal.draw(|frame| {
            let area = frame.area();
            let form_w = 52u16.min(area.width);
            let form_h = ((n + 3) as u16).min(area.height);
            let form_x = area.width.saturating_sub(form_w) / 2;
            let form_y = area.height.saturating_sub(form_h) / 2;
            let form_area = Rect::new(form_x, form_y, form_w, form_h);

            frame.render_widget(Clear, form_area);
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Audio Codecs ")
                .title_alignment(Alignment::Center);
            let inner = block.inner(form_area);
            frame.render_widget(block, form_area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);
            let list_area = chunks[0];
            let hint_area = chunks[1];

            for (i, e) in entries.iter().enumerate() {
                let y = list_area.y + i as u16;
                if y >= list_area.y + list_area.height {
                    break;
                }
                let focused_row = i == focused;
                let mark = if e.on { "[x]" } else { "[ ]" };
                let style = if focused_row {
                    Style::default()
                        .fg(theme.attention.get())
                        .add_modifier(Modifier::BOLD)
                } else if e.on {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(theme.subtle.get())
                };
                let marker = if focused_row { "\u{25b6} " } else { "  " };
                frame.render_widget(
                    Paragraph::new(Span::styled(format!("{marker}{mark} {}", e.label), style)),
                    Rect::new(list_area.x, y, list_area.width, 1),
                );
            }

            frame.render_widget(
                Paragraph::new(Span::styled(
                    "  Space toggle  Shift+\u{2191}\u{2193} reorder  Esc back",
                    Style::default().fg(theme.subtle.get()),
                )),
                hint_area,
            );
        })?;

        if let Event::Key(key) = event::read()? {
            let shift = key.modifiers.contains(KeyModifiers::SHIFT);
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char(' ') => {
                    if let Some(e) = entries.get_mut(focused) {
                        e.on = !e.on;
                    }
                }
                // Reorder (Shift+arrows); at an edge it's a no-op, not a wrap.
                KeyCode::Up if shift && focused > 0 => {
                    entries.swap(focused, focused - 1);
                    focused -= 1;
                }
                KeyCode::Down if shift && focused + 1 < n => {
                    entries.swap(focused, focused + 1);
                    focused += 1;
                }
                // Navigate (plain arrows, wrapping).
                KeyCode::Up if !shift && n > 0 => {
                    focused = if focused == 0 { n - 1 } else { focused - 1 };
                }
                KeyCode::Down if !shift && n > 0 => {
                    focused = (focused + 1) % n;
                }
                _ => {}
            }
        }
    }

    *codecs = entries
        .into_iter()
        .filter(|e| e.on)
        .map(|e| e.spec)
        .collect();
    Ok(())
}
