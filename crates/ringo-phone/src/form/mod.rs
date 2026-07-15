mod codecs;
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
use std::path::Path;

use crate::{config::Theme, profile::Profile};
use field::*;

use popups::run_discard_confirm;
pub use popups::{run_confirm, run_rename, run_restart_confirm};

type Term = Terminal<CrosstermBackend<crate::tui::TermWriter>>;

const LABEL_W: u16 = 15;
const SEP_W: u16 = 2;

// ─── FormState ───────────────────────────────────────────────────────────────

enum Action {
    None,
    Cancel,
    OpenHeaders,
    OpenCodecs,
    OpenEditor,
    Save,
}

struct FormState {
    fields: Vec<Field>,
    custom_headers: Vec<(String, String)>,
    audio_codecs: Vec<String>,
    active_tab: Tab,
    /// Focus position *within the active tab's fields* (not an index into `fields`).
    focused: usize,
    error: Option<String>,
    is_new: bool,
}

impl FormState {
    fn new(profile: &Profile, is_new: bool) -> Self {
        Self {
            fields: build_fields(profile, is_new),
            custom_headers: profile.custom_headers.clone(),
            audio_codecs: profile.audio_codecs.clone(),
            active_tab: Tab::Account,
            focused: 0,
            error: None,
            is_new,
        }
    }

    /// Indices into `self.fields` for the fields on the active tab, in order.
    fn tab_indices(&self) -> Vec<usize> {
        self.fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.group == self.active_tab)
            .map(|(i, _)| i)
            .collect()
    }

    /// Index into `self.fields` of the currently focused field.
    fn focused_index(&self) -> usize {
        self.tab_indices()[self.focused]
    }

    fn switch_tab(&mut self, forward: bool) {
        let n = Tab::ALL.len();
        let cur = Tab::ALL.iter().position(|t| *t == self.active_tab).unwrap();
        let next = if forward {
            (cur + 1) % n
        } else {
            (cur + n - 1) % n
        };
        self.active_tab = Tab::ALL[next];
        self.focused = 0;
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        self.error = None;
        let tab_len = self.tab_indices().len();
        // Focus positions: 0..tab_len are fields, then the Save and Cancel buttons.
        let n = tab_len + 2;
        let save_pos = tab_len;
        let cancel_pos = tab_len + 1;

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
            KeyCode::Tab => self.switch_tab(true),
            KeyCode::BackTab => self.switch_tab(false),
            KeyCode::Down => {
                self.focused = (self.focused + 1) % n;
            }
            KeyCode::Up => {
                self.focused = if self.focused == 0 {
                    n - 1
                } else {
                    self.focused - 1
                };
            }
            KeyCode::Enter => {
                if self.focused == save_pos {
                    return Action::Save;
                }
                if self.focused == cancel_pos {
                    return Action::Cancel;
                }
                if let FieldKind::SubMenu { .. } = &self.fields[self.focused_index()].kind {
                    return match self.fields[self.focused_index()].id {
                        FieldId::AudioCodecs => Action::OpenCodecs,
                        _ => Action::OpenHeaders,
                    };
                }
            }
            // On the button row, ← → move between Save and Cancel.
            KeyCode::Left | KeyCode::Right if self.focused >= tab_len => {
                self.focused = if self.focused == save_pos {
                    cancel_pos
                } else {
                    save_pos
                };
            }
            code if self.focused < tab_len => {
                let idx = self.focused_index();
                self.fields[idx].handle_key(code);
            }
            _ => {}
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

    fn update_codec_count(&mut self) {
        if let Some(f) = self
            .fields
            .iter_mut()
            .find(|f| f.id == FieldId::AudioCodecs)
        {
            if let FieldKind::SubMenu { count } = &mut f.kind {
                *count = self.audio_codecs.len();
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
            audio_codecs: self.audio_codecs.clone(),
            notify: get_toggle(self.field(Notify)),
            mwi: get_toggle(self.field(Mwi)),
            catchall: prev_profile.catchall,
            deflect: get_toggle(self.field(Deflect)),
            deflect_target: opt(get_text(self.field(DeflectTarget))),
            regint: {
                let s = get_text(self.field(Regint));
                let s = s.trim();
                if s.is_empty() {
                    None
                } else {
                    s.parse::<u32>().ok()
                }
            },
            custom_headers: self.custom_headers.clone(),
            metadata: prev_profile.metadata.clone(),
        };
        (name, profile)
    }

    /// Whether the form differs from the profile it was opened with — used to
    /// warn before discarding on cancel. For new profiles, a typed name counts.
    fn is_dirty(&self, base: &Profile) -> bool {
        let (name, profile) = self.extract(base);
        profile != *base || (self.is_new && !name.is_empty())
    }

    fn focus_field(&mut self, id: FieldId) {
        if let Some(f) = self.fields.iter().find(|f| f.id == id) {
            self.active_tab = f.group;
        }
        if let Some(pos) = self
            .tab_indices()
            .iter()
            .position(|&i| self.fields[i].id == id)
        {
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
        let regint = get_text(self.field(FieldId::Regint));
        if !regint.trim().is_empty() && regint.trim().parse::<u32>().is_err() {
            self.focus_field(FieldId::Regint);
            return Some("Reg. interval must be a whole number".into());
        }
        None
    }

    fn render(&self, frame: &mut Frame, title: &str, theme: &Theme) {
        let area = frame.area();
        // Box height covers the largest tab so it stays stable while switching.
        let max_tab_fields = Tab::ALL
            .iter()
            .map(|t| self.fields.iter().filter(|f| f.group == *t).count())
            .max()
            .unwrap_or(0);
        let form_w = 72u16.min(area.width);
        // tab bar (1) + separator (1) + fields + gap (1) + buttons (1) + gap (1)
        // + desc (1) + hint (1) + borders (2)
        let form_h = (max_tab_fields as u16 + 9).min(area.height);
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
            .constraints([
                Constraint::Length(1), // tab bar
                Constraint::Length(1), // separator
                Constraint::Min(1),    // fields
                Constraint::Length(1), // gap
                Constraint::Length(1), // Save / Cancel buttons
                Constraint::Length(1), // footer separator
                Constraint::Length(1), // description (tied to the focused field)
                Constraint::Length(1), // key hints
            ])
            .split(inner);

        let rule = |frame: &mut Frame, area: Rect| {
            frame.render_widget(
                Paragraph::new("─".repeat(area.width as usize))
                    .style(Style::default().fg(theme.subtle.get())),
                area,
            );
        };

        self.render_tab_bar(frame, chunks[0], theme);
        rule(frame, chunks[1]);

        let cursor_pos = self.render_fields(frame, chunks[2], theme);
        self.render_buttons(frame, chunks[4], theme);
        rule(frame, chunks[5]);
        self.render_footer(frame, chunks[6], chunks[7], theme);

        if let Some((cx, cy)) = cursor_pos {
            frame.set_cursor_position((cx, cy));
        }
    }

    fn render_tab_bar(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let mut spans = vec![Span::raw(" ")];
        for tab in Tab::ALL {
            let style = if *tab == self.active_tab {
                Style::default()
                    .fg(theme.attention.get())
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(theme.subtle.get())
            };
            spans.push(Span::styled(format!(" {} ", tab.label()), style));
            spans.push(Span::raw(" "));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Render the active tab's fields; returns the text cursor position if any.
    fn render_fields(&self, frame: &mut Frame, area: Rect, theme: &Theme) -> Option<(u16, u16)> {
        let indices = self.tab_indices();
        let visible = area.height as usize;
        // Focus may point past the last field (the Save/Cancel buttons live in their
        // own row); clamp to the field range so the button row never scrolls fields.
        let field_focus = self.focused.min(indices.len().saturating_sub(1));
        let scroll = (field_focus + 1).saturating_sub(visible);

        let value_x = area.x + LABEL_W + SEP_W;
        let value_w = area.width.saturating_sub(LABEL_W + SEP_W) as usize;
        let mut cursor_pos = None;

        for (pos, &fi) in indices.iter().enumerate() {
            if pos < scroll || pos >= scroll + visible {
                continue;
            }
            let field = &self.fields[fi];
            let y = area.y + (pos - scroll) as u16;
            let focused_here = pos == self.focused;

            let label_style = if focused_here {
                Style::default()
                    .fg(theme.attention.get())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.subtle.get())
            };
            // Right-align "* Label" as one unit so the asterisk hugs the label
            // instead of floating at the left edge.
            let labeled = if field.required {
                format!("* {}", field.label)
            } else {
                field.label.to_string()
            };
            let label_text = format!("{:>width$}  ", labeled, width = LABEL_W as usize);
            frame.render_widget(
                Paragraph::new(label_text.as_str()).style(label_style),
                Rect::new(area.x, y, LABEL_W + SEP_W, 1),
            );

            let value_area = Rect::new(value_x, y, value_w as u16, 1);
            if let Some(p) = field.render_value(frame, value_area, focused_here, theme) {
                cursor_pos = Some(p);
            }
        }
        cursor_pos
    }

    fn render_buttons(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let tab_len = self.tab_indices().len();
        // Both buttons are filled blocks (reversed) so they read as buttons at all
        // times; the focused one is the accent colour, the other a subtle grey.
        let button = |label: &str, focused: bool| {
            let colour = if focused {
                theme.accent.get()
            } else {
                theme.subtle.get()
            };
            let mut style = Style::default().fg(colour).add_modifier(Modifier::REVERSED);
            if focused {
                style = style.add_modifier(Modifier::BOLD);
            }
            Span::styled(format!("  {}  ", label), style)
        };
        let line = Line::from(vec![
            button("Save", self.focused == tab_len),
            Span::raw("   "),
            button("Cancel", self.focused == tab_len + 1),
        ]);
        frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
    }

    fn render_footer(&self, frame: &mut Frame, desc_area: Rect, hint_area: Rect, theme: &Theme) {
        let tab_len = self.tab_indices().len();
        // Description line, or the validation error (takes over the line) in red.
        let desc = if let Some(err) = &self.error {
            Span::styled(
                format!("  ✗ {}", err),
                Style::default().fg(theme.danger.get()),
            )
        } else {
            let text = if self.focused == tab_len {
                "Save changes and close."
            } else if self.focused == tab_len + 1 {
                "Discard changes and close."
            } else {
                self.fields[self.focused_index()].desc
            };
            Span::styled(
                format!("  {}", text),
                Style::default().fg(theme.subtle.get()),
            )
        };
        frame.render_widget(Paragraph::new(Line::from(desc)), desc_area);

        let hints: &[crate::tui::ui::Hint] = &[
            ("↑↓", "move"),
            ("⇥", "tab"),
            ("Space/←→", "change"),
            ("^S", "save"),
            ("^E", "editor"),
            ("Esc", "cancel"),
        ];
        crate::tui::ui::render_hint_bar(frame, hint_area, hints, theme);
    }
}

// ─── Profile form ────────────────────────────────────────────────────────────

fn build_fields(profile: &Profile, include_name: bool) -> Vec<Field> {
    use FieldId::*;
    let mut f = Vec::new();

    // ── Account ──────────────────────────────────────────────────────────────
    if include_name {
        f.push(
            Field::text(ProfileName, "Profile name", "", false, true)
                .group(Tab::Account)
                .desc("Local name for this profile (folder on disk); no slashes."),
        );
    }
    f.push(
        Field::text(
            DisplayName,
            "Display name",
            profile.display_name.as_deref().unwrap_or(""),
            false,
            false,
        )
        .group(Tab::Account)
        .desc("Name shown to the person you call."),
    );
    f.push(
        Field::text(Username, "Username", &profile.username, false, true)
            .group(Tab::Account)
            .desc("SIP user / phone number used to register."),
    );
    f.push(
        Field::text(Password, "Password", &profile.password, true, false)
            .group(Tab::Account)
            .desc("Password for the SIP account."),
    );
    f.push(
        Field::text(Domain, "Domain", &profile.domain, false, true)
            .group(Tab::Account)
            .desc("SIP domain of your provider, e.g. example.com."),
    );
    f.push(
        Field::text(
            AuthUser,
            "Auth user",
            profile.auth_user.as_deref().unwrap_or(""),
            false,
            false,
        )
        .group(Tab::Account)
        .desc("Auth username, only if it differs from the SIP user."),
    );
    f.push(
        Field::submenu(SipHeaders, "SIP Headers", profile.custom_headers.len())
            .group(Tab::Account)
            .desc("Custom headers added to this account's outgoing INVITEs."),
    );
    f.push(
        Field::text(
            Notes,
            "Notes",
            profile.notes.as_deref().unwrap_or(""),
            false,
            false,
        )
        .group(Tab::Account)
        .desc("Free-form note for yourself; never sent over SIP."),
    );

    // ── Network ──────────────────────────────────────────────────────────────
    f.push(
        Field::select(
            Transport,
            "Transport",
            TRANSPORTS,
            transport_idx(profile.transport.as_deref()),
        )
        .group(Tab::Network)
        .desc("Transport for SIP signalling (tls recommended)."),
    );
    f.push(
        Field::text(
            Outbound,
            "Outbound proxy",
            profile.outbound.as_deref().unwrap_or(""),
            false,
            false,
        )
        .group(Tab::Network)
        .desc("Route SIP via this proxy, e.g. sip:proxy.example.com;transport=tls."),
    );
    f.push(
        Field::text(
            StunServer,
            "STUN server",
            profile.stun_server.as_deref().unwrap_or(""),
            false,
            false,
        )
        .group(Tab::Network)
        .desc("STUN server for NAT traversal, e.g. stun:stun.example.com."),
    );
    f.push(
        Field::select(
            Encryption,
            "Encryption",
            ENCRYPTIONS,
            enc_idx(profile.media_enc.as_deref()),
        )
        .group(Tab::Network)
        .desc("Media encryption for the audio stream (SRTP/ZRTP)."),
    );
    f.push(
        Field::text(
            Regint,
            "Reg. interval",
            &profile.regint.map(|r| r.to_string()).unwrap_or_default(),
            false,
            false,
        )
        .group(Tab::Network)
        .desc("Re-register interval in seconds (empty = default 3600)."),
    );

    // ── Audio ────────────────────────────────────────────────────────────────
    f.push(
        Field::submenu(AudioCodecs, "Audio codecs", profile.audio_codecs.len())
            .group(Tab::Audio)
            .desc("Restrict/reorder the audio codecs offered (empty = default set)."),
    );

    // ── Features ─────────────────────────────────────────────────────────────
    f.push(
        Field::toggle(Notify, "Notifications", profile.notify)
            .group(Tab::Features)
            .desc("Show desktop notifications for calls and voicemail."),
    );
    f.push(
        Field::toggle(Mwi, "MWI", profile.mwi)
            .group(Tab::Features)
            .desc("Subscribe to message-waiting (voicemail) indication."),
    );

    // ── Forwarding ───────────────────────────────────────────────────────────
    f.push(
        Field::toggle(Deflect, "Call deflection", profile.deflect)
            .group(Tab::Forwarding)
            .desc("Deflect incoming calls with a 302 Moved Temporarily."),
    );
    f.push(
        Field::text(
            DeflectTarget,
            "Deflect target",
            profile.deflect_target.as_deref().unwrap_or(""),
            false,
            false,
        )
        .group(Tab::Forwarding)
        .desc("SIP URI or bare number (resolved to sip:<number>@<domain>)."),
    );

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
                Action::Cancel => {
                    // Warn before throwing away edits; a clean form cancels straight away.
                    if !state.is_dirty(&base_profile) || run_discard_confirm(terminal, theme)? {
                        return Ok(None);
                    }
                }
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
                Action::OpenCodecs => {
                    codecs::run_codecs_submenu(terminal, &mut state.audio_codecs, theme)?;
                    state.update_codec_count();
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
