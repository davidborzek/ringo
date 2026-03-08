use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

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

    pub(super) fn handle_contacts_key(&mut self, key: crossterm::event::KeyEvent) {
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
            KeyCode::Char('f') if key.modifiers == KeyModifiers::NONE => {
                self.contacts_state.search_query.clear();
                self.contacts_state.search_mode = false;
                self.contacts_state.show = false;
            }
            KeyCode::Char('/') if key.modifiers == KeyModifiers::NONE => {
                self.contacts_state.search_mode = true;
                self.contacts_state.search_query.clear();
                self.contacts_state.selected = 0;
            }
            KeyCode::Char('e') if key.modifiers == KeyModifiers::NONE => {
                self.open_contacts_editor();
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
            KeyCode::Enter => {
                if let Some(entry) = entries.get(self.contacts_state.selected) {
                    let number = self.contacts[entry.contact_idx].numbers[entry.number_idx].clone();
                    self.dial_set(number);
                    self.dial.mode = super::app::InputMode::Dial;
                    self.contacts_state.show = false;
                    self.contacts_state.search_query.clear();
                    self.contacts_state.search_mode = false;
                }
            }
            _ => {}
        }
    }

    fn open_contacts_editor(&mut self) {
        self.edit_contacts = true;
        self.quit = true;
    }
}

pub(super) fn render(f: &mut Frame, app: &super::app::App, area: Rect) {
    let entries = app.contacts_display_entries();
    let total_contacts = app.contacts.len();
    let filtered_len = entries.len();
    let visible = area.height.saturating_sub(2) as usize;

    let sel = if filtered_len > 0 {
        app.contacts_state.selected.min(filtered_len - 1)
    } else {
        0
    };
    let scroll = if sel < visible { 0 } else { sel - visible + 1 };

    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(fi, entry)| {
            let contact = &app.contacts[entry.contact_idx];
            let number = &contact.numbers[entry.number_idx];

            let is_first_number = entry.number_idx == 0;
            let name_part = if is_first_number {
                format!("{:<20}", contact.name)
            } else {
                " ".repeat(20)
            };

            let line = Line::from(vec![
                Span::styled(
                    format!(" {}", name_part),
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
    let title: Line = if app.contacts_state.search_mode {
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
            Span::styled("  (empty — press e to edit)", subtle),
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

    f.render_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::TOP)),
        area,
    );
}
