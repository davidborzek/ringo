use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::io;

use crate::{config::Theme, profile::Profile};

type Term = Terminal<CrosstermBackend<io::Stdout>>;

// ─── TextField ───────────────────────────────────────────────────────────────

struct TextField {
    chars: Vec<char>,
    cursor: usize,
}

impl TextField {
    fn new(s: &str) -> Self {
        let chars: Vec<char> = s.chars().collect();
        let cursor = chars.len();
        Self { chars, cursor }
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                self.chars.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.chars.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.chars.len() {
                    self.chars.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor < self.chars.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.chars.len(),
            _ => {}
        }
    }

    fn value(&self) -> String {
        self.chars.iter().collect()
    }

    /// Returns (visible_text, cursor_col_within_visible).
    fn render(&self, masked: bool, width: usize) -> (String, usize) {
        let display: Vec<char> = if masked {
            self.chars.iter().map(|_| '•').collect()
        } else {
            self.chars.clone()
        };
        let len = display.len();
        if len <= width {
            (display.iter().collect(), self.cursor)
        } else {
            let start = self.cursor.saturating_sub(width);
            let end = (start + width).min(len);
            (display[start..end].iter().collect(), self.cursor - start)
        }
    }
}

// ─── Field types ─────────────────────────────────────────────────────────────

const TRANSPORTS: &[&str] = &["default", "udp", "tcp", "tls"];
const ENCRYPTIONS: &[&str] = &["none", "dtls_srtp", "srtp", "srtp-mand", "zrtp"];

enum FieldKind {
    Text {
        tf: TextField,
        masked: bool,
    },
    Select {
        options: &'static [&'static str],
        idx: usize,
    },
    Toggle {
        value: bool,
    },
    SubMenu {
        count: usize,
    },
    Button,
}

struct Field {
    label: &'static str,
    required: bool,
    kind: FieldKind,
}

impl Field {
    fn text(label: &'static str, value: &str, masked: bool, required: bool) -> Self {
        Self {
            label,
            required,
            kind: FieldKind::Text {
                tf: TextField::new(value),
                masked,
            },
        }
    }
    fn select(label: &'static str, options: &'static [&'static str], idx: usize) -> Self {
        Self {
            label,
            required: false,
            kind: FieldKind::Select { options, idx },
        }
    }
    fn toggle(label: &'static str, value: bool) -> Self {
        Self {
            label,
            required: false,
            kind: FieldKind::Toggle { value },
        }
    }
    fn submenu(label: &'static str, count: usize) -> Self {
        Self {
            label,
            required: false,
            kind: FieldKind::SubMenu { count },
        }
    }
    fn button(label: &'static str) -> Self {
        Self {
            label,
            required: false,
            kind: FieldKind::Button,
        }
    }
}

// ─── Build / extract ─────────────────────────────────────────────────────────

fn transport_idx(t: Option<&str>) -> usize {
    match t {
        Some("udp") => 1,
        Some("tcp") => 2,
        Some("tls") => 3,
        _ => 0,
    }
}

fn enc_idx(e: Option<&str>) -> usize {
    match e {
        Some("dtls_srtp") => 1,
        Some("srtp") => 2,
        Some("srtp-mand") => 3,
        Some("zrtp") => 4,
        _ => 0,
    }
}

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

fn get_text(f: &Field) -> String {
    if let FieldKind::Text { tf, .. } = &f.kind {
        tf.value()
    } else {
        String::new()
    }
}
fn get_select(f: &Field) -> usize {
    if let FieldKind::Select { idx, .. } = &f.kind {
        *idx
    } else {
        0
    }
}
fn get_toggle(f: &Field) -> bool {
    if let FieldKind::Toggle { value } = &f.kind {
        *value
    } else {
        false
    }
}
fn opt(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
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

// ─── Layout constants ────────────────────────────────────────────────────────

const LABEL_W: u16 = 15;
const SEP_W: u16 = 2;

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
            // 2 borders + 1 hint + all fields
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

            // Scroll to keep focused field in view (pin to bottom)
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
                        run_headers_submenu(terminal, &mut custom_headers, theme)?;
                        // Update count in submenu field
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

// ─── SIP Headers submenu ─────────────────────────────────────────────────────

fn run_headers_submenu(
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
    let mut on_value = false; // false = key, true = value

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
            let sep_w = 3u16; // " = "
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
                    "  Tab key↔value  ↑↓ navigate  Ctrl+A add  Ctrl+D remove  Esc back",
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

    // Write back to headers map
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
