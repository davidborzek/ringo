mod field;
mod headers;
mod popups;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::{collections::HashMap, io, path::Path};

use crate::{config::Theme, profile::Profile};
use field::*;

pub use popups::{run_confirm, run_rename, run_restart_confirm};

type Term = Terminal<CrosstermBackend<io::Stdout>>;

const LABEL_W: u16 = 15;
const SEP_W: u16 = 2;

// ─── FormState ───────────────────────────────────────────────────────────────

enum Action {
    None,
    Cancel,
    OpenHeaders,
    OpenEditor,
    Save,
}

struct FormState {
    fields: Vec<Field>,
    custom_headers: HashMap<String, String>,
    focused: usize,
    error: Option<String>,
    is_new: bool,
}

impl FormState {
    fn new(profile: &Profile, is_new: bool) -> Self {
        Self {
            fields: build_fields(profile, is_new),
            custom_headers: profile.custom_headers.clone(),
            focused: 0,
            error: None,
            is_new,
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        self.error = None;
        let field_count = self.fields.len();

        match key.code {
            KeyCode::Esc => return Action::Cancel,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Action::Cancel;
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Action::Save;
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Action::OpenEditor;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.focused = (self.focused + 1) % field_count;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused = if self.focused == 0 {
                    field_count - 1
                } else {
                    self.focused - 1
                };
            }
            KeyCode::Enter => match &self.fields[self.focused].kind {
                FieldKind::SubMenu { .. } => return Action::OpenHeaders,
                FieldKind::Button => return Action::Save,
                _ => {}
            },
            code => {
                self.fields[self.focused].handle_key(code);
            }
        }
        Action::None
    }

    fn update_header_count(&mut self) {
        if let Some(f) = self.fields.iter_mut().find(|f| f.id == FieldId::SipHeaders) {
            if let FieldKind::SubMenu { count } = &mut f.kind {
                *count = self.custom_headers.len();
            }
        }
    }

    fn field(&self, id: FieldId) -> &Field {
        self.fields.iter().find(|f| f.id == id).unwrap()
    }

    fn extract(&self, prev_profile: &Profile) -> (String, Profile) {
        use FieldId::*;
        let name = if self.is_new {
            get_text(self.field(ProfileName))
        } else {
            String::new()
        };

        let transport_idx = get_select(self.field(Transport));
        let enc_idx = get_select(self.field(Encryption));

        let profile = Profile {
            display_name: opt(get_text(self.field(DisplayName))),
            username: get_text(self.field(Username)),
            password: get_text(self.field(Password)),
            domain: get_text(self.field(Domain)),
            transport: if transport_idx == 0 {
                None
            } else {
                Some(TRANSPORTS[transport_idx].into())
            },
            auth_user: opt(get_text(self.field(AuthUser))),
            outbound: opt(get_text(self.field(Outbound))),
            stun_server: opt(get_text(self.field(StunServer))),
            media_enc: if enc_idx == 0 {
                None
            } else {
                Some(ENCRYPTIONS[enc_idx].into())
            },
            notes: opt(get_text(self.field(Notes))),
            notify: get_toggle(self.field(Notify)),
            mwi: get_toggle(self.field(Mwi)),
            regint: prev_profile.regint,
            custom_headers: self.custom_headers.clone(),
            metadata: prev_profile.metadata.clone(),
        };
        (name, profile)
    }

    fn focus_field(&mut self, id: FieldId) {
        if let Some(pos) = self.fields.iter().position(|f| f.id == id) {
            self.focused = pos;
        }
    }

    fn validate(
        &mut self,
        name: &str,
        profile: &Profile,
        existing_names: &[String],
    ) -> Option<String> {
        if self.is_new {
            if name.is_empty() || name.contains('/') {
                self.focus_field(FieldId::ProfileName);
                return Some("non-empty, no slashes".into());
            }
            if existing_names.iter().any(|n| n == name) {
                self.focus_field(FieldId::ProfileName);
                return Some(format!("'{}' already exists", name));
            }
        }
        if profile.username.is_empty() {
            self.focus_field(FieldId::Username);
            return Some("Username is required".into());
        }
        if profile.domain.is_empty() {
            self.focus_field(FieldId::Domain);
            return Some("Domain is required".into());
        }
        None
    }

    fn render(&self, frame: &mut Frame, title: &str, theme: &Theme) {
        let area = frame.area();
        let field_count = self.fields.len();
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

        let scroll = if self.focused + 1 > visible {
            self.focused + 1 - visible
        } else {
            0
        };

        let value_x = fields_area.x + LABEL_W + SEP_W;
        let value_w = fields_area.width.saturating_sub(LABEL_W + SEP_W) as usize;
        let mut cursor_pos: Option<(u16, u16)> = None;

        for (i, field) in self.fields.iter().enumerate() {
            if i < scroll || i >= scroll + visible {
                continue;
            }
            let y = fields_area.y + (i - scroll) as u16;
            let focused_here = i == self.focused;

            // Label
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

            // Value
            let value_area = Rect::new(value_x, y, value_w as u16, 1);
            if let Some(pos) = field.render_value(frame, value_area, focused_here, theme) {
                cursor_pos = Some(pos);
            }
        }

        let hint = if let Some(err) = &self.error {
            Span::styled(
                format!("  ✗ {}", err),
                Style::default().fg(theme.danger.get()),
            )
        } else {
            Span::styled(
                "  ↑↓ Tab navigate  ← → Space toggle  Enter select  ^S save  ^E editor  Esc cancel",
                Style::default().fg(theme.subtle.get()),
            )
        };
        frame.render_widget(Paragraph::new(Line::from(hint)), hint_area);

        if let Some((cx, cy)) = cursor_pos {
            frame.set_cursor_position((cx, cy));
        }
    }
}

// ─── Profile form ────────────────────────────────────────────────────────────

fn build_fields(profile: &Profile, include_name: bool) -> Vec<Field> {
    use FieldId::*;
    let mut f = Vec::new();
    if include_name {
        f.push(Field::text(ProfileName, "Profile name", "", false, true));
    }
    f.push(Field::text(
        DisplayName,
        "Display name",
        profile.display_name.as_deref().unwrap_or(""),
        false,
        false,
    ));
    f.push(Field::text(
        Username,
        "Username",
        &profile.username,
        false,
        true,
    ));
    f.push(Field::text(
        Password,
        "Password",
        &profile.password,
        true,
        false,
    ));
    f.push(Field::text(Domain, "Domain", &profile.domain, false, true));
    f.push(Field::select(
        Transport,
        "Transport",
        TRANSPORTS,
        transport_idx(profile.transport.as_deref()),
    ));
    f.push(Field::text(
        AuthUser,
        "Auth user",
        profile.auth_user.as_deref().unwrap_or(""),
        false,
        false,
    ));
    f.push(Field::text(
        Outbound,
        "Outbound proxy",
        profile.outbound.as_deref().unwrap_or(""),
        false,
        false,
    ));
    f.push(Field::text(
        StunServer,
        "STUN server",
        profile.stun_server.as_deref().unwrap_or(""),
        false,
        false,
    ));
    f.push(Field::select(
        Encryption,
        "Encryption",
        ENCRYPTIONS,
        enc_idx(profile.media_enc.as_deref()),
    ));
    f.push(Field::text(
        Notes,
        "Notes",
        profile.notes.as_deref().unwrap_or(""),
        false,
        false,
    ));
    f.push(Field::toggle(Notify, "Notifications", profile.notify));
    f.push(Field::toggle(Mwi, "MWI", profile.mwi));
    f.push(Field::submenu(
        SipHeaders,
        "SIP Headers",
        profile.custom_headers.len(),
    ));
    f.push(Field::button(Save, "Save"));
    f
}

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
    let mut base_profile = profile.clone();
    let mut state = FormState::new(&base_profile, is_new);

    loop {
        terminal.draw(|frame| state.render(frame, title, theme))?;

        if let Event::Key(key) = event::read()? {
            match state.handle_key(key) {
                Action::Cancel => return Ok(None),
                Action::OpenEditor => {
                    if let Some(name) = profile_name {
                        let path = crate::profile::profile_dir(name)?.join("profile.toml");
                        open_editor(terminal, &path)?;
                        base_profile = crate::profile::load(name)?;
                        state = FormState::new(&base_profile, is_new);
                    }
                }
                Action::OpenHeaders => {
                    headers::run_headers_submenu(terminal, &mut state.custom_headers, theme)?;
                    state.update_header_count();
                }
                Action::Save => {
                    let (name, profile_out) = state.extract(&base_profile);
                    if let Some(err) = state.validate(&name, &profile_out, existing_names) {
                        state.error = Some(err);
                        continue;
                    }
                    let final_name = if is_new {
                        name
                    } else {
                        profile_name.unwrap().to_string()
                    };
                    return Ok(Some((final_name, profile_out)));
                }
                Action::None => {}
            }
        }
    }
}

fn open_editor(terminal: &mut Term, path: &Path) -> Result<()> {
    use crossterm::{
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    };
    use std::process::Command;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    let status = Command::new(&editor).arg(path).status();

    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;

    status
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("Failed to open editor '{}': {}", editor, e))
}
