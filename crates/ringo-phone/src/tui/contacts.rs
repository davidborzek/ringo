use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph},
};

use super::app::{ContactFormField, ContactFormMode};

/// Minimum width of the contact-name column.
const NAME_COL_MIN: usize = 12;

/// Truncate `s` to `width` display columns (char-aware, appending `…` when it
/// doesn't fit), otherwise left-pad it to `width` so the next column aligns.
fn fit(s: &str, width: usize) -> String {
    if s.chars().count() > width {
        let mut t: String = s.chars().take(width.saturating_sub(1)).collect();
        t.push('…');
        t
    } else {
        format!("{s:<width$}")
    }
}

/// A flattened entry for display: one row per number.
struct DisplayEntry {
    contact_idx: usize,
    number_idx: usize,
}

impl super::app::App {
    /// Flattened + filtered list of (contact_idx, number_idx) for the contacts overlay.
    fn contacts_display_entries(&self) -> Vec<DisplayEntry> {
        let q = self.contacts_state.search_query.to_lowercase();
        self.contacts
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                if q.is_empty() {
                    return true;
                }
                if c.name.to_lowercase().contains(&q) {
                    return true;
                }
                c.numbers.iter().any(|n| n.to_lowercase().contains(&q))
            })
            .flat_map(|(ci, c)| {
                c.numbers
                    .iter()
                    .enumerate()
                    .map(move |(ni, _)| DisplayEntry {
                        contact_idx: ci,
                        number_idx: ni,
                    })
            })
            .collect()
    }

    /// The contact index that the current selection belongs to.
    fn selected_contact_idx(&self) -> Option<usize> {
        let entries = self.contacts_display_entries();
        entries
            .get(self.contacts_state.selected)
            .map(|e| e.contact_idx)
    }

    pub(super) fn handle_contacts_key(&mut self, key: crossterm::event::KeyEvent) {
        // Delete confirmation popup captures all input.
        if let Some(ci) = self.contacts_state.delete_confirm {
            let mut do_delete = false;
            let mut close = false;
            match key.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                    self.confirm_yes = !self.confirm_yes;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => do_delete = true,
                KeyCode::Enter if self.confirm_yes => do_delete = true,
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    close = true;
                }
                _ => {}
            }
            if do_delete && ci < self.contacts.len() {
                self.contacts.remove(ci);
                crate::contacts::save(&self.contacts);
                let new_len = self.contacts_display_entries().len();
                if self.contacts_state.selected >= new_len && new_len > 0 {
                    self.contacts_state.selected = new_len - 1;
                }
            }
            if do_delete || close {
                self.contacts_state.delete_confirm = None;
                self.confirm_yes = false;
            }
            return;
        }

        // Form mode takes priority
        if self.contacts_state.form.mode != ContactFormMode::None {
            self.handle_contact_form_key(key);
            return;
        }

        if self.contacts_state.search_mode {
            match key.code {
                KeyCode::Esc => {
                    self.contacts_state.search_mode = false;
                    self.contacts_state.search_query.clear();
                    self.contacts_state.selected = 0;
                }
                KeyCode::Enter => {
                    self.contacts_state.search_mode = false;
                }
                KeyCode::Backspace => {
                    self.contacts_state.search_query.pop();
                    self.contacts_state.selected = 0;
                }
                KeyCode::Char(c)
                    if key.modifiers == KeyModifiers::NONE
                        || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.contacts_state.search_query.push(c);
                    self.contacts_state.selected = 0;
                }
                _ => {}
            }
            return;
        }

        let entries = self.contacts_display_entries();
        let len = entries.len();

        match key.code {
            KeyCode::Esc => {
                if !self.contacts_state.search_query.is_empty() {
                    self.contacts_state.search_query.clear();
                    self.contacts_state.selected = 0;
                } else {
                    self.contacts_state.show = false;
                }
            }
            KeyCode::Char('/') if key.modifiers == KeyModifiers::NONE => {
                self.contacts_state.search_mode = true;
                self.contacts_state.search_query.clear();
                self.contacts_state.selected = 0;
            }
            KeyCode::Char('a') if key.modifiers == KeyModifiers::NONE => {
                self.contacts_state.form.mode = ContactFormMode::Add;
                self.contacts_state.form.field = ContactFormField::Name;
                self.contacts_state.form.name.clear();
                self.contacts_state.form.numbers.clear();
                self.contacts_state.form.cursor = 0;
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::NONE => {
                if let Some(ci) = self.selected_contact_idx() {
                    let contact = &self.contacts[ci];
                    self.contacts_state.form.mode = ContactFormMode::Edit(ci);
                    self.contacts_state.form.field = ContactFormField::Name;
                    self.contacts_state.form.name = contact.name.clone();
                    self.contacts_state.form.numbers = contact.numbers.join(", ");
                    self.contacts_state.form.cursor = self.contacts_state.form.name.len();
                }
            }
            KeyCode::Char('d') if key.modifiers == KeyModifiers::NONE => {
                if let Some(ci) = self.selected_contact_idx() {
                    self.contacts_state.delete_confirm = Some(ci);
                    self.confirm_yes = false;
                }
            }
            KeyCode::Char('g') if key.modifiers == KeyModifiers::NONE => {
                self.contacts_state.selected = 0;
            }
            KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
                if len > 0 {
                    self.contacts_state.selected = len - 1;
                }
            }
            KeyCode::Up => {
                if self.contacts_state.selected > 0 {
                    self.contacts_state.selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.contacts_state.selected + 1 < len {
                    self.contacts_state.selected += 1;
                }
            }
            KeyCode::PageUp => {
                self.contacts_state.selected = self.contacts_state.selected.saturating_sub(10);
            }
            KeyCode::PageDown => {
                if len > 0 {
                    self.contacts_state.selected = (self.contacts_state.selected + 10).min(len - 1);
                }
            }
            KeyCode::Enter => {
                if let Some(entry) = entries.get(self.contacts_state.selected) {
                    let number = self.contacts[entry.contact_idx].numbers[entry.number_idx].clone();
                    match self.contacts_state.target {
                        super::app::ContactPickerTarget::Dial => {
                            self.dial_set(number);
                            self.dial.mode = super::app::InputMode::Dial;
                        }
                        super::app::ContactPickerTarget::Transfer => {
                            self.transfer_input_set(number);
                        }
                    }
                    self.contacts_state.show = false;
                    self.contacts_state.search_query.clear();
                    self.contacts_state.search_mode = false;
                }
            }
            _ => {}
        }
    }

    fn handle_contact_form_key(&mut self, key: crossterm::event::KeyEvent) {
        let form = &mut self.contacts_state.form;
        match key.code {
            KeyCode::Esc => {
                form.mode = ContactFormMode::None;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                form.field = match form.field {
                    ContactFormField::Name => ContactFormField::Numbers,
                    ContactFormField::Numbers => ContactFormField::Name,
                };
                form.cursor = match form.field {
                    ContactFormField::Name => form.name.len(),
                    ContactFormField::Numbers => form.numbers.len(),
                };
            }
            KeyCode::Enter => {
                let name = form.name.trim().to_string();
                if name.is_empty() {
                    form.mode = ContactFormMode::None;
                    return;
                }
                let numbers: Vec<String> = form
                    .numbers
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                let contact = crate::contacts::Contact { name, numbers };

                match form.mode {
                    ContactFormMode::Add => {
                        self.contacts.push(contact);
                    }
                    ContactFormMode::Edit(idx) => {
                        if idx < self.contacts.len() {
                            self.contacts[idx] = contact;
                        }
                    }
                    ContactFormMode::None => {}
                }

                crate::contacts::save(&self.contacts);
                form.mode = ContactFormMode::None;
            }
            KeyCode::Backspace => {
                let (buf, cursor) = form_buf_and_cursor(form);
                if *cursor > 0 {
                    let new = buf[..*cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    buf.remove(new);
                    *cursor = new;
                }
            }
            KeyCode::Delete => {
                let (buf, cursor) = form_buf_and_cursor(form);
                if *cursor < buf.len() {
                    buf.remove(*cursor);
                }
            }
            KeyCode::Left => {
                let (buf, cursor) = form_buf_and_cursor(form);
                if *cursor > 0 {
                    *cursor = buf[..*cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }
            KeyCode::Right => {
                let (buf, cursor) = form_buf_and_cursor(form);
                if *cursor < buf.len() {
                    let c = buf[*cursor..].chars().next().unwrap();
                    *cursor += c.len_utf8();
                }
            }
            KeyCode::Home => {
                form.cursor = 0;
            }
            KeyCode::End => {
                let (buf, cursor) = form_buf_and_cursor(form);
                *cursor = buf.len();
            }
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                let (buf, cursor) = form_buf_and_cursor(form);
                buf.insert(*cursor, c);
                *cursor += c.len_utf8();
            }
            _ => {}
        }
    }
}

fn form_buf_and_cursor(form: &mut super::app::ContactFormState) -> (&mut String, &mut usize) {
    match form.field {
        ContactFormField::Name => (&mut form.name, &mut form.cursor),
        ContactFormField::Numbers => (&mut form.numbers, &mut form.cursor),
    }
}

// ─── List rendering ──────────────────────────────────────────────────────────

pub(super) fn render(f: &mut Frame, app: &super::app::App, area: Rect) {
    let entries = app.contacts_display_entries();
    let total_contacts = app.contacts.len();
    let filtered_len = entries.len();
    // 2 border rows + 1 footer row.
    let visible = area.height.saturating_sub(3) as usize;

    let sel = if filtered_len > 0 {
        app.contacts_state.selected.min(filtered_len - 1)
    } else {
        0
    };
    let scroll = if sel < visible { 0 } else { sel - visible + 1 };

    // Size the name column to the longest name, but cap it so the number column
    // still fits (reserve ~24 cols for borders, gap and the number).
    let name_w = app
        .contacts
        .iter()
        .map(|c| c.name.chars().count())
        .max()
        .unwrap_or(0)
        .clamp(
            NAME_COL_MIN,
            (area.width as usize).saturating_sub(24).max(NAME_COL_MIN),
        );

    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(fi, entry)| {
            let contact = &app.contacts[entry.contact_idx];
            let number = &contact.numbers[entry.number_idx];

            let is_first_number = entry.number_idx == 0;
            // Fixed-width name column (truncated with an ellipsis) so the number
            // column always lines up; a trailing gap separates it from the number.
            let name_part = if is_first_number {
                fit(&contact.name, name_w)
            } else {
                " ".repeat(name_w)
            };

            let line = Line::from(vec![
                Span::styled(
                    format!(" {}  ", name_part),
                    Style::default()
                        .fg(app.theme.accent.get())
                        .add_modifier(if is_first_number {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::raw(number.as_str()),
            ]);

            let item = ListItem::new(line);
            if fi == sel {
                item.style(Style::default().bg(app.theme.subtle.get()))
            } else {
                item
            }
        })
        .collect();

    let accent = Style::default().fg(app.theme.accent.get());
    let subtle = Style::default().fg(app.theme.subtle.get());
    let title: Line = if app.contacts_state.form.mode != ContactFormMode::None {
        Line::from(vec![Span::styled("Contacts", accent)])
    } else if app.contacts_state.search_mode {
        Line::from(vec![
            Span::styled("Contacts", accent),
            Span::styled(
                format!("  / {}_", app.contacts_state.search_query),
                Style::default().fg(app.theme.attention.get()),
            ),
        ])
    } else if !app.contacts_state.search_query.is_empty() {
        Line::from(vec![
            Span::styled("Contacts", accent),
            Span::styled(
                format!(
                    "  /{} ({}/{})",
                    app.contacts_state.search_query, filtered_len, total_contacts
                ),
                subtle,
            ),
        ])
    } else if total_contacts == 0 {
        Line::from(vec![
            Span::styled("Contacts", accent),
            Span::styled("  (empty — press a to add)", subtle),
        ])
    } else {
        Line::from(vec![
            Span::styled("Contacts", accent),
            Span::styled(
                format!(
                    " ({}/{})",
                    if filtered_len > 0 { sel + 1 } else { 0 },
                    filtered_len
                ),
                subtle,
            ),
        ])
    };

    let search_footer = [("Enter", "confirm"), ("Esc", "clear")];
    let nav_footer = [
        ("↑↓", "nav"),
        ("PgUp/PgDn", "page"),
        ("Enter", "dial"),
        ("/", "search"),
        ("a", "add"),
        ("e", "edit"),
        ("d", "del"),
        ("Esc", "close"),
    ];
    // The add/edit form overlay draws its own hints, so no footer then.
    let footer: &[super::ui::Hint] = if app.contacts_state.form.mode != ContactFormMode::None {
        &[]
    } else if app.contacts_state.search_mode {
        &search_footer
    } else {
        &nav_footer
    };

    f.render_widget(Clear, area);
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let footer_h = if inner.height >= 2 && !footer.is_empty() {
        1
    } else {
        0
    };
    let list_area = Rect::new(inner.x, inner.y, inner.width, inner.height - footer_h);
    f.render_widget(List::new(items), list_area);
    if footer_h == 1 {
        f.render_widget(
            Paragraph::new(super::ui::styled_hints(footer, &app.theme)),
            Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1),
        );
    }
    super::ui::render_scrollbar(f, &app.theme, area, filtered_len, visible, scroll);

    // Form overlay
    if app.contacts_state.form.mode != ContactFormMode::None {
        render_contact_form(f, app, area);
    }
}

// ─── Form rendering ──────────────────────────────────────────────────────────

fn render_contact_form(f: &mut Frame, app: &super::app::App, area: Rect) {
    let form = &app.contacts_state.form;

    let popup_h: u16 = 7;
    let popup_w = area.width.saturating_sub(4).clamp(30, 60);
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    };

    f.render_widget(Clear, popup_area);

    let title = match form.mode {
        ContactFormMode::Add => " New Contact ",
        ContactFormMode::Edit(_) => " Edit Contact ",
        ContactFormMode::None => "",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.accent.get()));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // name label + input
            Constraint::Length(1), // spacer
            Constraint::Length(1), // numbers label + input
            Constraint::Length(1), // hint
            Constraint::Min(0),
        ])
        .split(inner);

    let active_style = Style::default().fg(app.theme.accent.get());
    let inactive_style = Style::default().fg(app.theme.subtle.get());

    // Name field
    let name_active = form.field == ContactFormField::Name;
    let name_style = if name_active {
        active_style
    } else {
        inactive_style
    };
    let name_label = Span::styled(" Name:    ", name_style);
    let name_value = Span::raw(&form.name);
    f.render_widget(
        Paragraph::new(Line::from(vec![name_label, name_value])),
        chunks[0],
    );

    if name_active {
        let cursor = form.cursor.min(form.name.len());
        let char_count = form.name[..cursor].chars().count();
        let cursor_x = chunks[0].x + 10 + char_count as u16;
        f.set_cursor_position((cursor_x, chunks[0].y));
    }

    // Numbers field
    let num_active = form.field == ContactFormField::Numbers;
    let num_style = if num_active {
        active_style
    } else {
        inactive_style
    };
    let num_label = Span::styled(" Numbers: ", num_style);
    let num_value = Span::raw(&form.numbers);
    f.render_widget(
        Paragraph::new(Line::from(vec![num_label, num_value])),
        chunks[2],
    );

    if num_active {
        let cursor = form.cursor.min(form.numbers.len());
        let char_count = form.numbers[..cursor].chars().count();
        let cursor_x = chunks[2].x + 10 + char_count as u16;
        f.set_cursor_position((cursor_x, chunks[2].y));
    }

    // Hint
    f.render_widget(
        Paragraph::new(" [Tab] switch  [Enter] save  [Esc] cancel")
            .style(Style::default().fg(app.theme.subtle.get())),
        chunks[3],
    );
}
