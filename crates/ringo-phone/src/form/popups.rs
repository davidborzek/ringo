use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::Term;
use crate::config::Theme;

// ─── Restart confirm popup ───────────────────────────────────────────────────

pub fn run_restart_confirm(terminal: &mut Term, theme: &Theme) -> Result<bool> {
    let mut confirm_yes = true;

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let w = 50u16.min(area.width);
            let h = 7u16.min(area.height);
            let popup = Rect::new(
                area.width.saturating_sub(w) / 2,
                area.height.saturating_sub(h) / 2,
                w,
                h,
            );
            frame.render_widget(Clear, popup);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Profile saved ")
                .title_alignment(Alignment::Center);
            let inner = block.inner(popup);
            frame.render_widget(block, popup);

            frame.render_widget(
                Paragraph::new("Restart now to apply changes?").alignment(Alignment::Center),
                Rect::new(inner.x, inner.y + 1, inner.width, 1),
            );

            let btn_y = inner.y + 3;
            let btn_x = inner.x + inner.width.saturating_sub(19) / 2;

            let later_style = if !confirm_yes {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let restart_style = if confirm_yes {
                Style::default()
                    .fg(Color::White)
                    .bg(theme.accent.get())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            frame.render_widget(
                Paragraph::new(Span::styled(" Later ", later_style)),
                Rect::new(btn_x, btn_y, 7, 1),
            );
            frame.render_widget(
                Paragraph::new(Span::styled("  Restart  ", restart_style)),
                Rect::new(btn_x + 11, btn_y, 10, 1),
            );
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => return Ok(false),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(false);
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                    confirm_yes = !confirm_yes;
                }
                KeyCode::Enter => return Ok(confirm_yes),
                KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
                KeyCode::Char('n') | KeyCode::Char('N') => return Ok(false),
                _ => {}
            }
        }
    }
}

// ─── Rename popup ────────────────────────────────────────────────────────────

pub fn run_rename(
    terminal: &mut Term,
    old_name: &str,
    existing: &[String],
    theme: &Theme,
) -> Result<Option<String>> {
    let mut input = old_name.to_string();
    let mut cursor = input.len();
    let mut error: Option<String> = None;

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let w = 50u16.min(area.width);
            let h = if error.is_some() { 9 } else { 7 };
            let h = h.min(area.height);
            let popup = Rect::new(
                area.width.saturating_sub(w) / 2,
                area.height.saturating_sub(h) / 2,
                w,
                h,
            );
            frame.render_widget(Clear, popup);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Rename Profile ")
                .title_alignment(Alignment::Center);
            let inner = block.inner(popup);
            frame.render_widget(block, popup);

            // Input field
            let input_area = Rect::new(inner.x + 1, inner.y + 1, inner.width - 2, 1);
            frame.render_widget(
                Paragraph::new(input.as_str()).style(
                    Style::default()
                        .fg(theme.accent.get())
                        .add_modifier(Modifier::BOLD),
                ),
                input_area,
            );

            // Cursor
            let cx = input_area.x + cursor as u16;
            if cx < input_area.x + input_area.width {
                frame.set_cursor_position((cx, input_area.y));
            }

            // Error message
            if let Some(err) = &error {
                let err_area = Rect::new(inner.x + 1, inner.y + 3, inner.width - 2, 1);
                frame.render_widget(
                    Paragraph::new(format!("✗ {}", err))
                        .style(Style::default().fg(theme.danger.get())),
                    err_area,
                );
            }

            // Hint
            let hint_y = inner.y + inner.height.saturating_sub(1);
            frame.render_widget(
                Paragraph::new("  Enter confirm  ·  Esc cancel")
                    .style(Style::default().fg(theme.subtle.get())),
                Rect::new(inner.x, hint_y, inner.width, 1),
            );
        })?;

        if let Event::Key(key) = event::read()? {
            error = None;
            match key.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                KeyCode::Enter => {
                    let name = input.trim().to_string();
                    if name.is_empty() {
                        error = Some("Name cannot be empty".into());
                    } else if name.contains('/') {
                        error = Some("Name cannot contain '/'".into());
                    } else if name == old_name {
                        return Ok(None);
                    } else if existing.iter().any(|n| n == &name) {
                        error = Some(format!("Profile '{}' already exists", name));
                    } else {
                        return Ok(Some(name));
                    }
                }
                KeyCode::Char(c) => {
                    input.insert(cursor, c);
                    cursor += 1;
                }
                KeyCode::Backspace => {
                    if cursor > 0 {
                        cursor -= 1;
                        input.remove(cursor);
                    }
                }
                KeyCode::Delete => {
                    if cursor < input.len() {
                        input.remove(cursor);
                    }
                }
                KeyCode::Left => {
                    cursor = cursor.saturating_sub(1);
                }
                KeyCode::Right => {
                    if cursor < input.len() {
                        cursor += 1;
                    }
                }
                KeyCode::Home => cursor = 0,
                KeyCode::End => cursor = input.len(),
                _ => {}
            }
        }
    }
}

// ─── Delete confirm popup ────────────────────────────────────────────────────

pub fn run_confirm(terminal: &mut Term, name: &str, theme: &Theme) -> Result<bool> {
    let mut confirm_yes = false;

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let w = 44u16.min(area.width);
            let h = 7u16.min(area.height);
            let popup = Rect::new(
                area.width.saturating_sub(w) / 2,
                area.height.saturating_sub(h) / 2,
                w,
                h,
            );
            frame.render_widget(Clear, popup);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Delete Profile ")
                .title_alignment(Alignment::Center);
            let inner = block.inner(popup);
            frame.render_widget(block, popup);

            frame.render_widget(
                Paragraph::new(format!("Delete '{}'?", name)).alignment(Alignment::Center),
                Rect::new(inner.x, inner.y + 1, inner.width, 1),
            );

            let btn_y = inner.y + 3;
            let btn_x = inner.x + inner.width.saturating_sub(17) / 2;

            let no_style = if !confirm_yes {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let yes_style = if confirm_yes {
                Style::default()
                    .fg(Color::White)
                    .bg(theme.danger.get())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            frame.render_widget(
                Paragraph::new(Span::styled("  No  ", no_style)),
                Rect::new(btn_x, btn_y, 6, 1),
            );
            frame.render_widget(
                Paragraph::new(Span::styled("  Yes  ", yes_style)),
                Rect::new(btn_x + 11, btn_y, 7, 1),
            );
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => return Ok(false),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(false);
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                    confirm_yes = !confirm_yes;
                }
                KeyCode::Enter => return Ok(confirm_yes),
                KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
                KeyCode::Char('n') | KeyCode::Char('N') => return Ok(false),
                _ => {}
            }
        }
    }
}
