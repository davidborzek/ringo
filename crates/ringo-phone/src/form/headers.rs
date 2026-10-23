use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::Term;
use super::field::TextField;
use crate::config::Theme;
use crate::mouse::{
    ClickAction, HitMap, Input, ListId, Pointed, Pointer, WHEEL_ROWS, offset_for_focus,
    step_clamped,
};

/// A click on row `index` picks the row and, by its column, the key or the
/// value half. The second press of a double click keeps what the first picked:
/// the redraw in between may have scrolled the list, so the same cell can name
/// another row by then.
fn click_row(
    focused: &mut usize,
    on_value: &mut bool,
    index: usize,
    col: u16,
    value_col: u16,
    double: bool,
) {
    if double {
        return;
    }
    *focused = index;
    *on_value = col >= value_col;
}

pub(crate) fn run_headers_submenu(
    terminal: &mut Term,
    headers: &mut Vec<(String, String)>,
    theme: &Theme,
    title: &str,
    mouse: bool,
) -> Result<()> {
    let mut entries: Vec<(TextField, TextField)> = headers
        .iter()
        .map(|(k, v)| (TextField::new(k), TextField::new(v)))
        .collect();
    let mut focused: usize = 0;
    let mut on_value = false;
    let mut pointer = Pointer::new(mouse);
    // First column of the value half, as drawn last: a click right of it focuses
    // the value, left of it the key.
    let mut value_col = 0u16;

    loop {
        let entry_count = entries.len();

        pointer.draw(terminal, |frame, hits| {
            value_col = draw_headers(frame, &entries, (focused, on_value), theme, title, hits);
        })?;

        let key = match pointer.read()? {
            Input::Key(key) => key,
            Input::Redraw => continue,
            Input::Pointer(Pointed::Wheel {
                list: ListId::Headers,
                down,
            }) => {
                focused = step_clamped(focused, entries.len(), down, WHEEL_ROWS);
                continue;
            }
            Input::Pointer(Pointed::Click {
                action:
                    ClickAction::Row {
                        list: ListId::Headers,
                        index,
                    },
                col,
                double,
                ..
            }) => {
                click_row(&mut focused, &mut on_value, index, col, value_col, double);
                continue;
            }
            Input::Pointer(_) => continue,
        };
        match key.code {
            KeyCode::Esc => break,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                entries.push((TextField::new(""), TextField::new("")));
                focused = entries.len() - 1;
                on_value = false;
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !entries.is_empty() {
                    entries.remove(focused);
                    if focused >= entries.len() && focused > 0 {
                        focused -= 1;
                    }
                }
            }
            KeyCode::Tab => {
                if entries.is_empty() {
                    continue;
                }
                on_value = !on_value;
            }
            KeyCode::Down => {
                if !entries.is_empty() {
                    focused = (focused + 1) % entry_count;
                }
            }
            KeyCode::Up => {
                if !entries.is_empty() {
                    focused = if focused == 0 {
                        entry_count - 1
                    } else {
                        focused - 1
                    };
                }
            }
            code => {
                if let Some((key_tf, val_tf)) = entries.get_mut(focused) {
                    if on_value {
                        val_tf.handle_key(code);
                    } else {
                        key_tf.handle_key(code);
                    }
                }
            }
        }
    }

    headers.clear();
    for (key_tf, val_tf) in &entries {
        let k = key_tf.value();
        let v = val_tf.value();
        if !k.is_empty() {
            headers.push((k, v));
        }
    }

    Ok(())
}

/// Draws the submenu and returns the first column of the value half.
fn draw_headers(
    frame: &mut Frame,
    entries: &[(TextField, TextField)],
    (focused, on_value): (usize, bool),
    theme: &Theme,
    title: &str,
    hits: &mut HitMap,
) -> u16 {
    hits.clear();
    let area = frame.area();
    let form_w = 72u16.min(area.width);
    let form_h = ((entries.len().max(1) + 3) as u16).min(area.height);
    let form_x = area.width.saturating_sub(form_w) / 2;
    let form_y = area.height.saturating_sub(form_h) / 2;
    let form_area = Rect::new(form_x, form_y, form_w, form_h);

    frame.render_widget(Clear, form_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .title_alignment(Alignment::Center);
    let inner = block.inner(form_area);
    frame.render_widget(block, form_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let fields_area = chunks[0];
    let hint_area = chunks[1];
    let key_w = 25u16.min(fields_area.width / 3);
    let sep_w = 3u16;
    let val_x = fields_area.x + key_w + sep_w;
    let val_w = fields_area.width.saturating_sub(key_w + sep_w);
    hits.push(
        fields_area,
        ClickAction::Scroll {
            list: ListId::Headers,
        },
    );
    let mut cursor_pos: Option<(u16, u16)> = None;

    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new("  (no headers — Ctrl+A to add)")
                .style(Style::default().fg(theme.subtle.get())),
            fields_area,
        );
    }

    let offset = offset_for_focus(focused, fields_area.height as usize);
    for (row, (i, (key_tf, val_tf))) in entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(fields_area.height as usize)
        .enumerate()
    {
        let y = fields_area.y + row as u16;
        hits.push(
            Rect::new(fields_area.x, y, fields_area.width, 1),
            ClickAction::Row {
                list: ListId::Headers,
                index: i,
            },
        );
        let is_focused = i == focused;
        let key_focused = is_focused && !on_value;
        let val_focused = is_focused && on_value;

        let key_style = if key_focused {
            Style::default()
                .fg(theme.attention.get())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let val_style = if val_focused {
            Style::default()
                .fg(theme.attention.get())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let (key_text, key_col) = key_tf.render(false, key_w as usize);
        let (val_text, val_col) = val_tf.render(false, val_w as usize);

        frame.render_widget(
            Paragraph::new(Span::styled(key_text, key_style)),
            Rect::new(fields_area.x, y, key_w, 1),
        );
        frame.render_widget(
            Paragraph::new(Span::styled(" = ", Style::default().fg(theme.subtle.get()))),
            Rect::new(fields_area.x + key_w, y, sep_w, 1),
        );
        frame.render_widget(
            Paragraph::new(Span::styled(val_text, val_style)),
            Rect::new(val_x, y, val_w, 1),
        );

        if key_focused {
            cursor_pos = Some((fields_area.x + key_col as u16, y));
        } else if val_focused {
            cursor_pos = Some((val_x + val_col as u16, y));
        }
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            "  Tab key\u{2194}value  \u{2191}\u{2193} navigate  Ctrl+A add  Ctrl+D remove  Esc back",
            Style::default().fg(theme.subtle.get()),
        )),
        hint_area,
    );

    if let Some((cx, cy)) = cursor_pos {
        frame.set_cursor_position((cx, cy));
    }

    val_x
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn the_focused_header_is_drawn_and_clickable_past_the_visible_rows() {
        let entries: Vec<(TextField, TextField)> = (0..8)
            .map(|i| (TextField::new(&format!("X-Key-{i}")), TextField::new("v")))
            .collect();
        // 8 entries need 11 rows; 8 rows leave 5 for the list.
        let mut terminal = Terminal::new(TestBackend::new(80, 8)).unwrap();
        let mut hits = HitMap::default();
        let theme = Theme::default();
        terminal
            .draw(|f| {
                draw_headers(f, &entries, (7, false), &theme, "Headers", &mut hits);
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let rows: Vec<String> = (0..8u16)
            .map(|y| (0..80u16).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        let y = rows.iter().position(|l| l.contains("X-Key-7")).unwrap() as u16;
        assert!(!rows.iter().any(|l| l.contains("X-Key-0")));
        let x = rows[y as usize].find("X-Key-7").unwrap() as u16;
        assert_eq!(
            hits.hit(x, y),
            Some(&ClickAction::Row {
                list: ListId::Headers,
                index: 7
            })
        );
    }

    #[test]
    fn a_double_click_keeps_the_row_the_first_click_focused() {
        let entries: Vec<(TextField, TextField)> = (0..8)
            .map(|i| (TextField::new(&format!("X-Key-{i}")), TextField::new("v")))
            .collect();
        // 8 entries need 11 rows; 8 rows leave 5 for the list.
        let mut terminal = Terminal::new(TestBackend::new(80, 8)).unwrap();
        let mut hits = HitMap::default();
        let theme = Theme::default();
        let mut focused = 7usize;
        let mut on_value = false;
        let mut value_col = 0u16;

        let top_row_index = |hits: &HitMap| match hits.hit(6, 1) {
            Some(ClickAction::Row { index, .. }) => *index,
            other => panic!("the top list row must be clickable: {other:?}"),
        };

        terminal
            .draw(|f| {
                value_col = draw_headers(
                    f,
                    &entries,
                    (focused, on_value),
                    &theme,
                    "Headers",
                    &mut hits,
                );
            })
            .unwrap();
        let first = top_row_index(&hits);
        click_row(&mut focused, &mut on_value, first, 6, value_col, false);
        assert_eq!(first, 3, "entry 7 scrolled: the top row shows entry 3");

        // The focus the first click set may have shifted the viewport, so the
        // same cell names another row now.
        terminal
            .draw(|f| {
                value_col = draw_headers(
                    f,
                    &entries,
                    (focused, on_value),
                    &theme,
                    "Headers",
                    &mut hits,
                );
            })
            .unwrap();
        let second = top_row_index(&hits);
        assert_eq!(second, 0, "the viewport scrolled back to the top");
        click_row(&mut focused, &mut on_value, second, 6, value_col, true);
        assert_eq!(focused, 3, "the double click keeps the first click's row");
    }
}
