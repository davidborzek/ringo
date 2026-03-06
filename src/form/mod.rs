mod field;
mod headers;
mod popups;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::io;

use crate::{config::Theme, profile::Profile};
use field::*;

pub use popups::{run_confirm, run_restart_confirm};

type Term = Terminal<CrosstermBackend<io::Stdout>>;

// ─── Layout constants ────────────────────────────────────────────────────────

const LABEL_W: u16 = 15;
const SEP_W: u16 = 2;

// ─── Build / extract ─────────────────────────────────────────────────────────

fn build_fields(profile: &Profile, include_name: bool) -> Vec<Field> {
    let mut f = Vec::new();
    if include_name {
        f.push(Field::text("Profile name", "", false, true));
    }
    f.push(Field::text(
        "Display name",
        profile.display_name.as_deref().unwrap_or(""),
        false,
        false,
    ));
    f.push(Field::text("Username", &profile.username, false, true));
    f.push(Field::text("Password", &profile.password, true, false));
    f.push(Field::text("Domain", &profile.domain, false, true));
    f.push(Field::select(
        "Transport",
        TRANSPORTS,
        transport_idx(profile.transport.as_deref()),
    ));
    f.push(Field::text(
        "Auth user",
        profile.auth_user.as_deref().unwrap_or(""),
        false,
        false,
    ));
    f.push(Field::text(
        "Outbound proxy",
        profile.outbound.as_deref().unwrap_or(""),
        false,
        false,
    ));
    f.push(Field::text(
        "STUN server",
        profile.stun_server.as_deref().unwrap_or(""),
        false,
        false,
    ));
    f.push(Field::select(
        "Encryption",
        ENCRYPTIONS,
        enc_idx(profile.media_enc.as_deref()),
    ));
    f.push(Field::toggle("Notifications", profile.notify));
    f.push(Field::toggle("MWI", profile.mwi));
    f.push(Field::submenu("SIP Headers", profile.custom_headers.len()));
    f.push(Field::button("Save"));
    f
}

fn extract_profile(
    fields: &[Field],
    include_name: bool,
    prev_profile: &Profile,
) -> (String, Profile) {
    let mut i = 0;
    let name = if include_name {
        let n = get_text(&fields[i]);
        i += 1;
        n
    } else {
        String::new()
    };

    let profile = Profile {
        display_name: {
            let v = opt(get_text(&fields[i]));
            i += 1;
            v
        },
        username: {
            let v = get_text(&fields[i]);
            i += 1;
            v
        },
        password: {
            let v = get_text(&fields[i]);
            i += 1;
            v
        },
        domain: {
            let v = get_text(&fields[i]);
            i += 1;
            v
        },
        transport: {
            let v = get_select(&fields[i]);
            i += 1;
            if v == 0 {
                None
            } else {
                Some(TRANSPORTS[v].into())
            }
        },
        auth_user: {
            let v = opt(get_text(&fields[i]));
            i += 1;
            v
        },
        outbound: {
            let v = opt(get_text(&fields[i]));
            i += 1;
            v
        },
        stun_server: {
            let v = opt(get_text(&fields[i]));
            i += 1;
            v
        },
        media_enc: {
            let v = get_select(&fields[i]);
            i += 1;
            if v == 0 {
                None
            } else {
                Some(ENCRYPTIONS[v].into())
            }
        },
        notify: {
            let v = get_toggle(&fields[i]);
            i += 1;
            v
        },
        mwi: get_toggle(&fields[i]),
        regint: prev_profile.regint,
        custom_headers: prev_profile.custom_headers.clone(),
    };
    (name, profile)
}

// ─── Profile form ────────────────────────────────────────────────────────────

/// Show an interactive form. `profile_name = None` means "New" (includes name field).
/// `existing_names` is used to reject duplicate profile names on create.
pub fn run_form(
    terminal: &mut Term,
    profile_name: Option<&str>,
    profile: &Profile,
    existing_names: &[String],
    theme: &Theme,
) -> Result<Option<(String, Profile)>> {
    let is_new = profile_name.is_none();
    let title = if is_new {
        " New Profile "
    } else {
        " Edit Profile "
    };
    let mut fields = build_fields(profile, is_new);
    let mut custom_headers = profile.custom_headers.clone();
    let mut focused: usize = 0;
    let mut error: Option<String> = None;

    loop {
        let field_count = fields.len();

        terminal.draw(|frame| {
            let area = frame.area();
            let form_w = 72u16.min(area.width);
            let form_h = (field_count as u16 + 3).min(area.height);
            let form_x = area.width.saturating_sub(form_w) / 2;
            let form_y = area.height.saturating_sub(form_h) / 2;
            let form_area = Rect::new(form_x, form_y, form_w, form_h);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_alignment(Alignment::Center);
            let inner = block.inner(form_area);
            frame.render_widget(block, form_area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);

            let fields_area = chunks[0];
            let hint_area = chunks[1];
            let visible = fields_area.height as usize;

            let scroll = if focused + 1 > visible {
                focused + 1 - visible
            } else {
                0
            };

            let value_x = fields_area.x + LABEL_W + SEP_W;
            let value_w = fields_area.width.saturating_sub(LABEL_W + SEP_W) as usize;
            let mut cursor_pos: Option<(u16, u16)> = None;

            for (i, field) in fields.iter().enumerate() {
                if i < scroll || i >= scroll + visible {
                    continue;
                }
                let y = fields_area.y + (i - scroll) as u16;
                let focused_here = i == focused;

                let label_style = if focused_here {
                    Style::default()
                        .fg(theme.attention.get())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.subtle.get())
                };
                let req_mark = if field.required { "* " } else { "  " };
                let label_text = format!(
                    "{}{:>width$}  ",
                    req_mark,
                    field.label,
                    width = LABEL_W.saturating_sub(2) as usize
                );
                frame.render_widget(
                    Paragraph::new(label_text.as_str()).style(label_style),
                    Rect::new(fields_area.x, y, LABEL_W + SEP_W, 1),
                );

                match &field.kind {
                    FieldKind::Text { tf, masked } => {
                        let (text, col) = tf.render(*masked, value_w);
                        let span = if text.is_empty() && !focused_here {
                            Span::styled("·", Style::default().fg(theme.subtle.get()))
                        } else {
                            Span::styled(
                                text,
                                if focused_here {
                                    Style::default()
                                } else {
                                    Style::default().fg(Color::White)
                                },
                            )
                        };
                        frame.render_widget(
                            Paragraph::new(Line::from(span)),
                            Rect::new(value_x, y, value_w as u16, 1),
                        );
                        if focused_here {
                            cursor_pos = Some((value_x + col as u16, y));
                        }
                    }
                    FieldKind::Select { options, idx } => {
                        let text = format!("◀ {} ▶", options[*idx]);
                        let style = if focused_here {
                            Style::default().fg(theme.attention.get())
                        } else {
                            Style::default().fg(Color::White)
                        };
                        frame.render_widget(
                            Paragraph::new(Span::styled(text, style)),
                            Rect::new(value_x, y, value_w as u16, 1),
                        );
                    }
                    FieldKind::Toggle { value } => {
                        let (icon, color) = if *value {
                            ("● on", theme.success.get())
                        } else {
                            ("○ off", theme.subtle.get())
                        };
                        let style = Style::default().fg(color);
                        let style = if focused_here {
                            style.add_modifier(Modifier::BOLD)
                        } else {
                            style
                        };
                        frame.render_widget(
                            Paragraph::new(Span::styled(icon, style)),
                            Rect::new(value_x, y, value_w as u16, 1),
                        );
                    }
                    FieldKind::SubMenu { count } => {
                        let text = format!("({}) ▶", count);
                        let style = if focused_here {
                            Style::default()
                                .fg(theme.attention.get())
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        frame.render_widget(
                            Paragraph::new(Span::styled(text, style)),
                            Rect::new(value_x, y, value_w as u16, 1),
                        );
                    }
                    FieldKind::Button => {
                        let style = if focused_here {
                            Style::default()
                                .fg(Color::White)
                                .bg(theme.accent.get())
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.subtle.get())
                        };
                        frame.render_widget(
                            Paragraph::new(Span::styled(format!("  {}  ", field.label), style)),
                            Rect::new(value_x, y, value_w as u16, 1),
                        );
                    }
                }
            }

            let hint = if let Some(err) = &error {
                Span::styled(
                    format!("  ✗ {}", err),
                    Style::default().fg(theme.danger.get()),
                )
            } else {
                Span::styled(
                    "  ↑↓ Tab navigate  ← → Space toggle  Enter select  Esc cancel",
                    Style::default().fg(theme.subtle.get()),
                )
            };
            frame.render_widget(Paragraph::new(Line::from(hint)), hint_area);

            if let Some((cx, cy)) = cursor_pos {
                frame.set_cursor_position((cx, cy));
            }
        })?;

        if let Event::Key(key) = event::read()? {
            error = None;
            match key.code {
                KeyCode::Esc => return Ok(None),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                KeyCode::Tab | KeyCode::Down => {
                    focused = (focused + 1) % field_count;
                }
                KeyCode::BackTab | KeyCode::Up => {
                    focused = if focused == 0 {
                        field_count - 1
                    } else {
                        focused - 1
                    };
                }
                KeyCode::Enter => match &fields[focused].kind {
                    FieldKind::SubMenu { .. } => {
                        headers::run_headers_submenu(terminal, &mut custom_headers, theme)?;
                        if let FieldKind::SubMenu { count } = &mut fields[focused].kind {
                            *count = custom_headers.len();
                        }
                    }
                    FieldKind::Button => {
                        let (name, mut profile_out) = extract_profile(&fields, is_new, profile);
                        profile_out.custom_headers = custom_headers.clone();
                        if is_new {
                            if name.is_empty() || name.contains('/') || name.contains(' ') {
                                error = Some("non-empty, no spaces or slashes".into());
                                focused = 0;
                                continue;
                            }
                            if existing_names.iter().any(|n| n == &name) {
                                error = Some(format!("'{}' already exists", name));
                                focused = 0;
                                continue;
                            }
                        }
                        if profile_out.username.is_empty() {
                            error = Some("Username is required".into());
                            focused = if is_new { 2 } else { 1 };
                            continue;
                        }
                        if profile_out.domain.is_empty() {
                            error = Some("Domain is required".into());
                            focused = if is_new { 4 } else { 3 };
                            continue;
                        }
                        let final_name = if is_new {
                            name
                        } else {
                            profile_name.unwrap().to_string()
                        };
                        return Ok(Some((final_name, profile_out)));
                    }
                    _ => {}
                },
                code => match &mut fields[focused].kind {
                    FieldKind::Text { tf, .. } => tf.handle_key(code),
                    FieldKind::Select { options, idx } => match code {
                        KeyCode::Left => {
                            *idx = if *idx == 0 {
                                options.len() - 1
                            } else {
                                *idx - 1
                            };
                        }
                        KeyCode::Right | KeyCode::Char(' ') => {
                            *idx = (*idx + 1) % options.len();
                        }
                        _ => {}
                    },
                    FieldKind::Toggle { value } => match code {
                        KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => *value = !*value,
                        _ => {}
                    },
                    _ => {}
                },
            }
        }
    }
}
