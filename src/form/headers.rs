use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::Term;
use super::field::TextField;
use crate::config::Theme;

pub(crate) fn run_headers_submenu(
    terminal: &mut Term,
    headers: &mut std::collections::HashMap<String, String>,
    theme: &Theme,
) -> Result<()> {
    let mut entries: Vec<(TextField, TextField)> = {
        let mut keys: Vec<&String> = headers.keys().collect();
        keys.sort();
        keys.into_iter()
            .map(|k| (TextField::new(k), TextField::new(&headers[k])))
            .collect()
    };
    let mut focused: usize = 0;
    let mut on_value = false;

    loop {
        let entry_count = entries.len();

        terminal.draw(|frame| {
            let area = frame.area();
            let form_w = 72u16.min(area.width);
            let form_h = ((entry_count.max(1) + 3) as u16).min(area.height);
            let form_x = area.width.saturating_sub(form_w) / 2;
            let form_y = area.height.saturating_sub(form_h) / 2;
            let form_area = Rect::new(form_x, form_y, form_w, form_h);

            frame.render_widget(Clear, form_area);
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" SIP Headers ")
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
            let mut cursor_pos: Option<(u16, u16)> = None;

            if entries.is_empty() {
                frame.render_widget(
                    Paragraph::new("  (no headers — Ctrl+A to add)")
                        .style(Style::default().fg(theme.subtle.get())),
                    fields_area,
                );
            }

            for (i, (key_tf, val_tf)) in entries.iter().enumerate() {
                let y = fields_area.y + i as u16;
                if y >= fields_area.y + fields_area.height {
                    break;
                }
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
                    Paragraph::new(Span::styled(
                        " = ",
                        Style::default().fg(theme.subtle.get()),
                    )),
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
        })?;

        if let Event::Key(key) = event::read()? {
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
    }

    headers.clear();
    for (key_tf, val_tf) in &entries {
        let k = key_tf.value();
        let v = val_tf.value();
        if !k.is_empty() {
            headers.insert(k, v);
        }
    }

    Ok(())
}
