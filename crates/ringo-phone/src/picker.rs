use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use crate::config::Theme;

pub enum PickerAction {
    Start(String),
    Edit(String),
    Clone(String),
    Delete(String),
    Rename(String),
    New,
    Settings,
    /// User left the picker without choosing a profile (Esc / Ctrl+C). Not an
    /// error — the caller exits cleanly.
    Quit,
}

/// A profile entry shown in the picker. `subtitle` may be empty.
pub struct PickerItem {
    pub name: String,
    pub subtitle: String,
}

const LOGO: &[&str] = &[
    "██████╗ ██╗███╗  ██╗ ██████╗  ██████╗ ",
    "██╔══██╗██║████╗ ██║██╔════╝ ██╔═══██╗",
    "██████╔╝██║██╔██╗██║██║  ███╗██║   ██║",
    "██╔══██╗██║██║╚████║██║   ██║██║   ██║",
    "██║  ██║██║██║ ╚███║╚██████╔╝╚██████╔╝",
    "╚═╝  ╚═╝╚═╝╚═╝  ╚══╝ ╚═════╝  ╚═════╝",
];

/// Keybind hints shown at the bottom of the picker.
const PICKER_HINTS: &[(&str, &str)] = &[
    ("Enter", "start"),
    ("^E", "edit"),
    ("^R", "rename"),
    ("^Y", "clone"),
    ("^D", "delete"),
    ("^N", "new"),
    ("^S", "settings"),
    ("Esc", "quit"),
];

/// Run the profile picker using an existing terminal (no terminal lifecycle management).
/// When `focus` is provided the picker pre-selects the item with that name.
pub(crate) fn run(
    terminal: &mut crate::tui::Term,
    items: &[PickerItem],
    theme: &Theme,
    focus: Option<&str>,
) -> Result<PickerAction> {
    let mut query = String::new();
    let mut selected: usize = focus
        .and_then(|name| items.iter().position(|i| i.name == name))
        .unwrap_or(0);

    loop {
        let filtered: Vec<&PickerItem> = items
            .iter()
            .filter(|i| fuzzy_match(&query, &i.name))
            .collect();

        if !filtered.is_empty() && selected >= filtered.len() {
            selected = filtered.len() - 1;
        }

        terminal.draw(|frame| {
            let area = frame.area();
            let header_height = (area.height / 4).max(8);
            let hint_h = crate::tui::ui::hint_rows(PICKER_HINTS, area.width);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(header_height), // [0] logo
                    Constraint::Length(3),             // [1] search box
                    Constraint::Length(1),             // [2] gap
                    Constraint::Min(1),                // [3] list box
                    Constraint::Length(hint_h),        // [4] hint
                ])
                .split(area);

            // ASCII logo + version — vertically centered in the header area.
            let version_line = Line::from(Span::styled(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                Style::default().fg(theme.subtle.get()),
            ));
            let logo_height = LOGO.len() as u16 + 1;
            let top_pad = chunks[0].height.saturating_sub(logo_height) / 2;
            let mut logo_lines: Vec<Line> = std::iter::repeat_n(Line::from(""), top_pad as usize)
                .chain(LOGO.iter().map(|l| Line::from(*l)))
                .chain(std::iter::once(version_line))
                .collect();
            while logo_lines.len() < chunks[0].height as usize {
                logo_lines.push(Line::from(""));
            }
            frame.render_widget(
                Paragraph::new(logo_lines)
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(theme.accent.get())),
                chunks[0],
            );

            // Search box
            frame.render_widget(
                Paragraph::new(query.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .padding(Padding::horizontal(1))
                        .title(" 🔍 search "),
                ),
                chunks[1],
            );
            // Cursor after the border (1) + left padding (1) + typed query.
            frame.set_cursor_position((
                chunks[1].x + 2 + query.chars().count() as u16,
                chunks[1].y + 1,
            ));

            // Pad the name to the widest one so the aor subtitles line up.
            let name_w = filtered
                .iter()
                .map(|i| i.name.chars().count())
                .max()
                .unwrap_or(0);

            // Profile list — selection styling handled by the List widget
            let list_items: Vec<ListItem> = filtered
                .iter()
                .map(|item| {
                    if item.subtitle.is_empty() {
                        ListItem::new(Line::from(item.name.as_str()))
                    } else {
                        ListItem::new(Line::from(vec![
                            Span::raw(format!("{:<name_w$}", item.name)),
                            Span::styled(
                                format!("  {}", item.subtitle),
                                Style::default().fg(theme.subtle.get()),
                            ),
                        ]))
                    }
                })
                .collect();

            let list_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .padding(Padding::horizontal(1))
                .title(" profiles ")
                .title_style(Style::default().fg(theme.accent.get()));

            if filtered.is_empty() {
                // Empty state: no profiles at all, or none matching the query.
                let inner = list_block.inner(chunks[3]);
                frame.render_widget(list_block, chunks[3]);
                let msg = if query.trim().is_empty() {
                    "  no profiles yet — press Ctrl+N to create one".to_string()
                } else {
                    format!("  no profiles match \"{}\"", query)
                };
                frame.render_widget(
                    Paragraph::new(msg).style(Style::default().fg(theme.subtle.get())),
                    inner,
                );
            } else {
                let mut list_state = ListState::default();
                list_state.select(Some(selected));
                frame.render_stateful_widget(
                    List::new(list_items)
                        .block(list_block)
                        .highlight_style(
                            Style::default()
                                .fg(theme.accent.get())
                                .add_modifier(Modifier::BOLD),
                        )
                        .highlight_symbol("▶ "),
                    chunks[3],
                    &mut list_state,
                );
            }

            // Hint line (wraps onto extra rows on narrow terminals)
            frame.render_widget(
                Paragraph::new(crate::tui::ui::styled_hints(PICKER_HINTS, theme))
                    .wrap(Wrap { trim: false }),
                chunks[4],
            );
        })?;

        if let Event::Key(key) = event::read()? {
            let ctrl = key.modifiers == KeyModifiers::CONTROL;
            match key.code {
                KeyCode::Esc => return Ok(PickerAction::Quit),
                KeyCode::Char('c') if ctrl => return Ok(PickerAction::Quit),
                KeyCode::Char('n') if ctrl => return Ok(PickerAction::New),
                KeyCode::Char('s') if ctrl => return Ok(PickerAction::Settings),
                KeyCode::Char('e') if ctrl => {
                    if let Some(item) = filtered.get(selected) {
                        return Ok(PickerAction::Edit(item.name.clone()));
                    }
                }
                KeyCode::Char('y') if ctrl => {
                    if let Some(item) = filtered.get(selected) {
                        return Ok(PickerAction::Clone(item.name.clone()));
                    }
                }
                KeyCode::Char('r') if ctrl => {
                    if let Some(item) = filtered.get(selected) {
                        return Ok(PickerAction::Rename(item.name.clone()));
                    }
                }
                KeyCode::Char('d') if ctrl => {
                    if let Some(item) = filtered.get(selected) {
                        return Ok(PickerAction::Delete(item.name.clone()));
                    }
                }
                KeyCode::Enter => {
                    if let Some(item) = filtered.get(selected) {
                        return Ok(PickerAction::Start(item.name.clone()));
                    }
                }
                KeyCode::Up => {
                    if selected > 0 {
                        selected -= 1;
                    } else if !filtered.is_empty() {
                        selected = filtered.len() - 1;
                    }
                }
                KeyCode::Down => {
                    if !filtered.is_empty() && selected + 1 < filtered.len() {
                        selected += 1;
                    } else {
                        selected = 0;
                    }
                }
                KeyCode::Backspace => {
                    query.pop();
                    selected = 0;
                }
                KeyCode::Char(c) => {
                    query.push(c);
                    selected = 0;
                }
                _ => {}
            }
        }
    }
}

fn fuzzy_match(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let text_lower = text.to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|token| text_lower.contains(token))
}
