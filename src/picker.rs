use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::io;

use crate::config::Theme;

pub enum PickerAction {
    Start(String),
    Edit(String),
    Clone(String),
    Delete(String),
    New,
    Settings,
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

/// Run the profile picker using an existing terminal (no terminal lifecycle management).
pub(crate) fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    items: &[PickerItem],
    theme: &Theme,
) -> Result<PickerAction> {
    let mut query = String::new();
    let mut selected: usize = 0;

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
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(header_height),
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(area);

            // ASCII logo — vertically centered in header area
            let logo_height = LOGO.len() as u16;
            let top_pad = chunks[0].height.saturating_sub(logo_height) / 2;
            let mut logo_lines: Vec<Line> = std::iter::repeat_n(Line::from(""), top_pad as usize)
                .chain(LOGO.iter().map(|l| Line::from(*l)))
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

            // Search input
            frame.render_widget(
                Paragraph::new(query.as_str())
                    .block(Block::default().borders(Borders::ALL).title(" 🔍 search ")),
                chunks[1],
            );

            // Profile list — selection styling handled by the List widget
            let list_items: Vec<ListItem> = filtered
                .iter()
                .map(|item| {
                    if item.subtitle.is_empty() {
                        ListItem::new(Line::from(item.name.as_str()))
                    } else {
                        ListItem::new(Line::from(vec![
                            Span::raw(item.name.as_str()),
                            Span::styled(
                                format!("  {}", item.subtitle),
                                Style::default().fg(theme.subtle.get()),
                            ),
                        ]))
                    }
                })
                .collect();

            let mut list_state = ListState::default();
            list_state.select(if filtered.is_empty() {
                None
            } else {
                Some(selected)
            });
            frame.render_stateful_widget(
                List::new(list_items)
                    .block(Block::default().borders(Borders::ALL))
                    .highlight_style(
                        Style::default()
                            .fg(theme.accent.get())
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▶ "),
                chunks[2],
                &mut list_state,
            );

            // Hint line
            frame.render_widget(
                Paragraph::new(
                    "  Enter start  ·  ^E edit  ·  ^Y clone  ·  ^D delete  ·  ^N new  ·  ^S settings  ·  Esc quit",
                )
                .style(Style::default().fg(theme.subtle.get())),
                chunks[3],
            );
        })?;

        if let Event::Key(key) = event::read()? {
            let ctrl = key.modifiers == KeyModifiers::CONTROL;
            match key.code {
                KeyCode::Esc => anyhow::bail!("No selection made."),
                KeyCode::Char('c') if ctrl => anyhow::bail!("No selection made."),
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
    let text_lower: Vec<char> = text.to_lowercase().chars().collect();
    let mut t = 0;
    for qc in query.to_lowercase().chars() {
        match text_lower[t..].iter().position(|&c| c == qc) {
            Some(pos) => t += pos + 1,
            None => return false,
        }
    }
    true
}
