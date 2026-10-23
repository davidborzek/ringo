use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};
use ringo_core::event::CodecInfo;

use super::Term;
use crate::config::Theme;
use crate::mouse::{
    ClickAction, HitMap, Input, ListId, Pointed, Pointer, WHEEL_ROWS, offset_for_focus,
    step_clamped,
};

/// One picker row: the baresip codec spec to store, a display label, and whether
/// it's selected.
struct Entry {
    spec: String,
    label: String,
    on: bool,
}

/// What a click on row `index` does: the first click moves the focus to it, a
/// double click toggles it — Enter does nothing here, so the double click does
/// what activates a codec, Space. The toggle aims at the focused entry, not the
/// row the click names: the redraw after the first click may have scrolled the
/// list, so the same cell can name another row by then.
fn click_row(entries: &mut [Entry], focused: &mut usize, index: usize, double: bool) {
    if double {
        if let Some(e) = entries.get_mut(*focused) {
            e.on = !e.on;
        }
    } else {
        *focused = index;
    }
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
    mouse: bool,
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
    let mut pointer = Pointer::new(mouse);

    loop {
        let n = entries.len();
        pointer.draw(terminal, |frame, hits| {
            draw_codecs(frame, &entries, focused, theme, hits)
        })?;

        let key = match pointer.read()? {
            Input::Key(key) => Some(key),
            Input::Redraw => None,
            Input::Pointer(Pointed::Wheel {
                list: ListId::Codecs,
                down,
            }) => {
                focused = step_clamped(focused, n, down, WHEEL_ROWS);
                None
            }
            Input::Pointer(Pointed::Click {
                action:
                    ClickAction::Row {
                        list: ListId::Codecs,
                        index,
                    },
                double,
                ..
            }) => {
                click_row(&mut entries, &mut focused, index, double);
                None
            }
            Input::Pointer(Pointed::Click {
                action: ClickAction::Key(key),
                ..
            }) => Some(key),
            Input::Pointer(Pointed::Click {
                action: ClickAction::Dismiss,
                ..
            }) => break,
            Input::Pointer(_) => None,
        };
        if let Some(key) = key {
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

fn draw_codecs(
    frame: &mut Frame,
    entries: &[Entry],
    focused: usize,
    theme: &Theme,
    hits: &mut HitMap,
) {
    hits.clear();
    let area = frame.area();
    let form_w = 52u16.min(area.width);
    let form_h = ((entries.len() + 3) as u16).min(area.height);
    let form_x = area.width.saturating_sub(form_w) / 2;
    let form_y = area.height.saturating_sub(form_h) / 2;
    let form_area = Rect::new(form_x, form_y, form_w, form_h);
    // A click beside the submenu closes it, as Esc does.
    hits.push(area, ClickAction::Dismiss);
    hits.push(form_area, ClickAction::Inert);

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
    hits.push(
        list_area,
        ClickAction::Scroll {
            list: ListId::Codecs,
        },
    );

    let offset = offset_for_focus(focused, list_area.height as usize);
    for (row, (i, e)) in entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
    {
        let y = list_area.y + row as u16;
        hits.push(
            Rect::new(list_area.x, y, list_area.width, 1),
            ClickAction::Row {
                list: ListId::Codecs,
                index: i,
            },
        );
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

    crate::tui::ui::render_hint_bar(frame, hint_area, CODEC_HINTS, theme);
    crate::tui::ui::record_hint_hits(hits, hint_area, CODEC_HINTS, false);
}

const CODEC_HINTS: &[crate::tui::ui::Hint] =
    &[("Space", "toggle"), ("⇧↑/⇧↓", "reorder"), ("Esc", "back")];

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn the_hint_line_is_a_clickable_hint_bar() {
        let entries = vec![Entry {
            spec: "opus/48000/2".into(),
            label: "opus".into(),
            on: false,
        }];
        let mut terminal = Terminal::new(TestBackend::new(60, 10)).unwrap();
        let mut hits = HitMap::default();
        terminal
            .draw(|f| draw_codecs(f, &entries, 0, &Theme::default(), &mut hits))
            .unwrap();

        let buf = terminal.backend().buffer();
        let rows: Vec<String> = (0..10u16)
            .map(|y| (0..60u16).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        // Cells, not bytes: the hint line has multi-byte arrows.
        let at = |needle: &str| {
            let y = rows.iter().position(|l| l.contains(needle)).unwrap();
            let byte = rows[y].find(needle).unwrap();
            (rows[y][..byte].chars().count() as u16, y as u16)
        };
        let key = |needle: &str| {
            let (x, y) = at(needle);
            hits.hit(x, y).cloned()
        };
        let plain = |code| ClickAction::Key(KeyEvent::new(code, KeyModifiers::NONE));
        assert_eq!(key("Space"), Some(plain(KeyCode::Char(' '))));
        assert_eq!(key("Esc"), Some(plain(KeyCode::Esc)));
        assert_eq!(
            key("⇧↑"),
            Some(ClickAction::Key(KeyEvent::new(
                KeyCode::Up,
                KeyModifiers::SHIFT
            )))
        );
    }

    #[test]
    fn the_focused_codec_is_drawn_and_clickable_past_the_visible_rows() {
        let entries: Vec<Entry> = (0..8)
            .map(|i| Entry {
                spec: format!("c{i}/8000/1"),
                label: format!("codec-{i}"),
                on: false,
            })
            .collect();
        // 8 entries need 11 rows; 8 rows leave 5 for the list.
        let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
        let mut hits = HitMap::default();
        let theme = Theme::default();
        terminal
            .draw(|f| draw_codecs(f, &entries, 7, &theme, &mut hits))
            .unwrap();

        let buf = terminal.backend().buffer();
        let rows: Vec<String> = (0..8u16)
            .map(|y| (0..60u16).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        let y = rows.iter().position(|l| l.contains("codec-7")).unwrap() as u16;
        assert!(!rows.iter().any(|l| l.contains("codec-0")));
        let x = rows[y as usize].find("codec-7").unwrap() as u16;
        assert_eq!(
            hits.hit(x, y),
            Some(&ClickAction::Row {
                list: ListId::Codecs,
                index: 7
            })
        );
    }

    #[test]
    fn a_double_click_toggles_the_row_the_first_click_focused() {
        let mut entries: Vec<Entry> = (0..8)
            .map(|i| Entry {
                spec: format!("c{i}/8000/1"),
                label: format!("codec-{i}"),
                on: false,
            })
            .collect();
        // 8 entries need 11 rows; 8 rows leave 5 for the list.
        let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
        let mut hits = HitMap::default();
        let theme = Theme::default();
        let mut focused = 7usize;

        let top_row_index = |hits: &HitMap| match hits.hit(10, 1) {
            Some(ClickAction::Row { index, .. }) => *index,
            other => panic!("the top list row must be clickable: {other:?}"),
        };

        terminal
            .draw(|f| draw_codecs(f, &entries, focused, &theme, &mut hits))
            .unwrap();
        let first = top_row_index(&hits);
        click_row(&mut entries, &mut focused, first, false);
        assert_eq!(first, 3, "entry 7 scrolled: the top row shows entry 3");

        // The focus the first click set may have shifted the viewport, so the
        // same cell names another row now.
        terminal
            .draw(|f| draw_codecs(f, &entries, focused, &theme, &mut hits))
            .unwrap();
        let second = top_row_index(&hits);
        assert_eq!(second, 0, "the viewport scrolled back to the top");
        click_row(&mut entries, &mut focused, second, true);
        assert_eq!(focused, 3, "the double click keeps the first click's row");
        assert!(entries[3].on, "the first click's codec is toggled");
        assert!(!entries[0].on, "the row now under the cell stays as it was");
    }
}
