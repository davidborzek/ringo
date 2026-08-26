use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use crate::config::{LogoMode, Theme};

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
    /// A session for this profile is already up. Starting a second one
    /// registers another contact with the provider, and the provider then
    /// decides which of them gets the call — worth seeing before you press
    /// Enter. A snapshot from when the list was built, not live.
    pub running: bool,
    /// The profile could not be read. Without this the row looks like any
    /// other and the failure only surfaces on start.
    pub broken: bool,
}

const LOGO: &[&str] = &[
    "██████╗ ██╗███╗  ██╗ ██████╗  ██████╗ ",
    "██╔══██╗██║████╗ ██║██╔════╝ ██╔═══██╗",
    "██████╔╝██║██╔██╗██║██║  ███╗██║   ██║",
    "██╔══██╗██║██║╚████║██║   ██║██║   ██║",
    "██║  ██║██║██║ ╚███║╚██████╔╝╚██████╔╝",
    "╚═╝  ╚═╝╚═╝╚═╝  ╚══╝ ╚═════╝  ╚═════╝",
];

/// The same word in one line, for terminals the block letters do not fit in.
const LOGO_SMALL: &[&str] = &["RINGO"];

/// The session-dot column in front of every name.
const GUTTER_W: usize = 2;
/// What `highlight_symbol("▶ ")` reserves in front of that, on every row.
const HIGHLIGHT_W: usize = 2;

/// Columns the block letters need. Narrower than this and they wrap into
/// nonsense, so width counts as "no room" just like height does.
const LOGO_WIDTH: u16 = 40;

/// Rows the picker needs below the header before the wordmark may claim any:
/// the search box, the gap under it, enough list to be worth showing, and the
/// hint bar. The logo yields to the list, not the other way round.
const SEARCH_H: u16 = 3;
const GAP_H: u16 = 1;
/// Not the least that renders, but the least worth having: the picker exists to
/// show profiles, and a dozen rows is the point where scrolling stops being the
/// normal case. With everything else this puts the block letters at 25 rows and
/// up — a standard 24-row terminal gets the one-liner, which is the intent.
const MIN_LIST_H: u16 = 12;

/// Rows the header occupies: the wordmark, the version line under it, and one
/// blank for air. Zero when there is no wordmark, so the picker then starts
/// straight at the search box.
fn header_height(logo: &[&str]) -> u16 {
    if logo.is_empty() {
        0
    } else {
        logo.len() as u16 + 2
    }
}

/// Which wordmark fits, given the terminal and what the user asked for.
/// An empty slice means none at all.
fn logo_for(mode: LogoMode, width: u16, height: u16, hint_h: u16) -> &'static [&'static str] {
    match mode {
        LogoMode::Off => &[],
        LogoMode::Full => LOGO,
        LogoMode::Small => LOGO_SMALL,
        LogoMode::Auto => {
            // header_height() so the budget matches what is actually drawn —
            // the version line and the air under it included.
            let needed = header_height(LOGO) + SEARCH_H + GAP_H + MIN_LIST_H + hint_h;
            if width >= LOGO_WIDTH && height >= needed {
                LOGO
            } else {
                LOGO_SMALL
            }
        }
    }
}

/// `text` cut to `max` columns, ending in an ellipsis when something was
/// dropped. Counted in chars, like the column widths around it.
///
/// Without this a long name and its subtitle simply run past the border and
/// ratatui clips them mid-word, which reads as broken rather than as shortened.
fn truncate(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
}

/// Where a navigation key moves the selection in a list of `len` entries.
/// `page` is the number of rows on screen, so PgUp/PgDn move by exactly what
/// you can see rather than by a guessed constant.
///
/// Up and Down wrap — with a filtered list that is usually the shorter way
/// round. Paging and Home/End clamp instead: they are for getting to a known
/// place, and wrapping would overshoot it.
fn move_selection(key: KeyCode, selected: usize, len: usize, page: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len - 1;
    match key {
        KeyCode::Up if selected > 0 => selected - 1,
        KeyCode::Up => last,
        KeyCode::Down if selected < last => selected + 1,
        KeyCode::Down => 0,
        KeyCode::PageUp => selected.saturating_sub(page.max(1)),
        KeyCode::PageDown => (selected + page.max(1)).min(last),
        // The query has no cursor to move — it is only ever appended to and
        // backspaced — so Home and End belong to the list.
        KeyCode::Home => 0,
        KeyCode::End => last,
        _ => selected,
    }
}

/// Keybind hints shown at the bottom of the picker.
/// No `Enter start`: picking a profile is what the picker is for, and the row
/// under the cursor is already marked with `▶`. Same reasoning as the arrow
/// keys, which are not listed either.
const PICKER_HINTS: &[(&str, &str)] = &[
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
    logo_mode: LogoMode,
    // Warnings to show above the list: a config that failed to load, or a
    // profile that could not be started or cloned.
    notices: &[String],
    focus: Option<&str>,
) -> Result<PickerAction> {
    let mut query = String::new();
    let mut selected: usize = focus
        .and_then(|name| items.iter().position(|i| i.name == name))
        .unwrap_or(0);

    // Rows the list shows, filled in by the first draw. Until then a guess, and
    // only PgUp/PgDn would notice.
    let mut page: usize = 10;
    loop {
        let filtered: Vec<&PickerItem> = items
            .iter()
            .filter(|i| row_matches(&query, &i.name, &i.subtitle))
            .collect();

        if !filtered.is_empty() && selected >= filtered.len() {
            selected = filtered.len() - 1;
        }

        terminal.draw(|frame| {
            let area = frame.area();
            let hint_h = crate::tui::ui::hint_rows(PICKER_HINTS, area.width);
            let logo = logo_for(logo_mode, area.width, area.height, hint_h);
            let header_height = header_height(logo);
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
            let logo_height = logo.len() as u16 + 1;
            let top_pad = chunks[0].height.saturating_sub(logo_height) / 2;
            let mut logo_lines: Vec<Line> = std::iter::repeat_n(Line::from(""), top_pad as usize)
                .chain(logo.iter().map(|l| Line::from(*l)))
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

            // The row between the search box and the list is a gap unless there
            // is something to say. Saying it here matters more than anywhere
            // else: the picker runs before the log file is opened, so a warning
            // has nowhere else to go, and a phone that silently ignores its
            // config looks broken rather than misconfigured.
            if let Some(first) = notices.first() {
                let more = notices.len() - 1;
                let extra = if more > 0 {
                    format!(" (+{more} more)")
                } else {
                    String::new()
                };
                frame.render_widget(
                    Paragraph::new(format!(" ⚠ {first}{extra}"))
                        .style(Style::default().fg(theme.danger.get())),
                    chunks[2],
                );
            }

            // Search box
            frame.render_widget(
                Paragraph::new(query.as_str()).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .padding(Padding::horizontal(1))
                        // No emoji in the title: terminals disagree on whether
                        // one is single- or double-width, and the border shifts
                        // with them.
                        .title(" search "),
                ),
                chunks[1],
            );
            // Cursor after the border (1) + left padding (1) + typed query.
            frame.set_cursor_position((
                chunks[1].x + 2 + query.chars().count() as u16,
                chunks[1].y + 1,
            ));

            // Count in the title: with a filter active there is otherwise no way
            // to tell whether the list is short or merely cut down.
            let title = if filtered.len() == items.len() {
                format!(" profiles ({}) ", items.len())
            } else {
                format!(" profiles ({}/{}) ", filtered.len(), items.len())
            };
            let list_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .padding(Padding::horizontal(1))
                .title(title)
                .title_style(Style::default().fg(theme.accent.get()));
            // What PgUp/PgDn move by, taken from the window actually drawn.
            page = list_block.inner(chunks[3]).height.max(1) as usize;

            // Measured over every profile, not the filtered ones: taken from the
            // filtered set the column would move on each keystroke as the
            // longest name drops out of the results, and the whole list twitches
            // while you type.
            let name_w = items
                .iter()
                .map(|i| i.name.chars().count())
                .max()
                .unwrap_or(0);

            // Split the row: two columns of gutter, then the name, then what is
            // left for the subtitle. A name too long for the row wins over its
            // subtitle — it is what identifies the profile.
            // highlight_symbol indents every row by its own width on top of the
            // gutter, so both come off the budget.
            let row_w = list_block.inner(chunks[3]).width as usize;
            let used = HIGHLIGHT_W + GUTTER_W;
            let name_col = name_w.min(row_w.saturating_sub(used));
            let sub_w = row_w.saturating_sub(used + name_col + 2);

            // Profile list — selection styling handled by the List widget
            let list_items: Vec<ListItem> = filtered
                .iter()
                .map(|item| {
                    // A dot for a live session, a blank otherwise, so the names
                    // stay in one column either way.
                    let mark = Span::styled(
                        if item.running { "● " } else { "  " },
                        Style::default().fg(theme.success.get()),
                    );
                    let name = Span::raw(format!("{:<name_col$}", truncate(&item.name, name_col)));
                    let trailing = if item.broken {
                        Span::styled(
                            format!("  {}", truncate("⚠ unreadable", sub_w)),
                            Style::default().fg(theme.danger.get()),
                        )
                    } else {
                        Span::styled(
                            format!("  {}", truncate(&item.subtitle, sub_w)),
                            Style::default().fg(theme.subtle.get()),
                        )
                    };
                    ListItem::new(Line::from(vec![mark, name, trailing]))
                })
                .collect();

            if filtered.is_empty() {
                // Empty state: no profiles at all, or none matching the query.
                let inner = list_block.inner(chunks[3]);
                frame.render_widget(list_block, chunks[3]);
                let msg = if query.trim().is_empty() {
                    "no profiles yet — press Ctrl+N to create one".to_string()
                } else {
                    format!("no profiles match \"{}\"", query)
                };
                // Centred, so it reads as a message about the box rather than
                // as a first entry that got cut off.
                let top_pad = inner.height.saturating_sub(1) / 2;
                let lines: Vec<Line> = std::iter::repeat_n(Line::from(""), top_pad as usize)
                    .chain(std::iter::once(Line::from(msg)))
                    .collect();
                frame.render_widget(
                    Paragraph::new(lines)
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(theme.subtle.get())),
                    inner,
                );
            } else {
                // Inner height before the block is moved into the List: that is
                // the viewport the scrollbar has to describe.
                let visible = list_block.inner(chunks[3]).height as usize;
                let mut list_state = ListState::default();
                list_state.select(Some(selected));
                frame.render_stateful_widget(
                    List::new(list_items)
                        .block(list_block)
                        // Not REVERSED: that inverts each span on its own, so
                        // the session dot, the name and the subtitle each turn
                        // into a differently coloured block and the row reads as
                        // a patchwork. An arrow plus accent leaves the colours
                        // alone.
                        .highlight_style(
                            Style::default()
                                .fg(theme.accent.get())
                                .add_modifier(Modifier::BOLD),
                        )
                        .highlight_symbol("▶ "),
                    chunks[3],
                    &mut list_state,
                );
                // After rendering, because the List is what decides how far it
                // had to scroll to keep the selection in view. No-ops when
                // everything fits.
                crate::tui::ui::render_scrollbar(
                    frame,
                    theme,
                    chunks[3],
                    filtered.len(),
                    visible,
                    list_state.offset(),
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
                KeyCode::Up
                | KeyCode::Down
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Home
                | KeyCode::End => {
                    selected = move_selection(key.code, selected, filtered.len(), page);
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

/// Whether a row answers `query`: every whitespace-separated token has to
/// appear in the name or in the subtitle, in any order and either field. So
/// "dev sipgate" finds a profile called `[DEV] Channel 01` registered at
/// sipgate, without either word having to be in the field you happened to
/// think of.
///
/// The subtitle counts because it is right there in the row — aor, domain, and
/// whatever `picker.info` adds. Searching only the name meant typing something
/// you could plainly see and getting nothing.
fn row_matches(query: &str, name: &str, subtitle: &str) -> bool {
    if query.trim().is_empty() {
        return true;
    }
    let (name, subtitle) = (name.to_lowercase(), subtitle.to_lowercase());
    query
        .to_lowercase()
        .split_whitespace()
        .all(|token| name.contains(token) || subtitle.contains(token))
}

#[cfg(test)]
mod tests {
    use super::{
        KeyCode, LOGO, LOGO_SMALL, LogoMode, PickerItem, logo_for, move_selection, row_matches,
        truncate,
    };

    /// The shortest terminal the block letters are allowed on, by the rule in
    /// logo_for, with a one-row hint bar.
    const ROOMY_H: u16 =
        LOGO.len() as u16 + 2 + super::SEARCH_H + super::GAP_H + super::MIN_LIST_H + 1;

    fn item(name: &str, subtitle: &str) -> PickerItem {
        PickerItem {
            name: name.into(),
            subtitle: subtitle.into(),
            running: false,
            broken: false,
        }
    }

    /// The picker's own filter, as the render loop applies it.
    fn matching<'a>(query: &str, items: &'a [PickerItem]) -> Vec<&'a str> {
        items
            .iter()
            .filter(|i| row_matches(query, &i.name, &i.subtitle))
            .map(|i| i.name.as_str())
            .collect()
    }

    #[test]
    fn truncation_marks_what_it_dropped() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("exactly-10", 10), "exactly-10");
        assert_eq!(
            truncate("[DEV] Channel 03 - Pickup test", 12),
            "[DEV] Chann…"
        );
        assert_eq!(truncate("abc", 1), "…");
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn truncation_counts_characters_not_bytes() {
        // The column widths around it count chars, and a multi-byte name must
        // not be cut through the middle of one.
        assert_eq!(truncate("Büro-Ähre", 5), "Büro…");
        assert_eq!(truncate("Büro", 4), "Büro");
    }

    #[test]
    fn up_and_down_wrap_around() {
        // With a filtered list, the way round is usually the shorter one.
        assert_eq!(move_selection(KeyCode::Up, 0, 5, 3), 4);
        assert_eq!(move_selection(KeyCode::Down, 4, 5, 3), 0);
        assert_eq!(move_selection(KeyCode::Up, 3, 5, 3), 2);
        assert_eq!(move_selection(KeyCode::Down, 3, 5, 3), 4);
    }

    #[test]
    fn paging_clamps_instead_of_wrapping() {
        // Paging is for getting somewhere known; wrapping would overshoot it.
        assert_eq!(move_selection(KeyCode::PageDown, 8, 20, 5), 13);
        assert_eq!(move_selection(KeyCode::PageDown, 18, 20, 5), 19);
        assert_eq!(move_selection(KeyCode::PageUp, 3, 20, 5), 0);
        assert_eq!(move_selection(KeyCode::PageUp, 0, 20, 5), 0);
    }

    #[test]
    fn paging_moves_by_what_is_on_screen() {
        assert_eq!(move_selection(KeyCode::PageDown, 0, 100, 12), 12);
        assert_eq!(move_selection(KeyCode::PageDown, 0, 100, 30), 30);
    }

    #[test]
    fn home_and_end_go_to_the_ends() {
        assert_eq!(move_selection(KeyCode::Home, 7, 20, 5), 0);
        assert_eq!(move_selection(KeyCode::End, 7, 20, 5), 19);
    }

    #[test]
    fn an_empty_list_stays_at_zero() {
        // A filter that matches nothing must not produce an index into nothing.
        for key in [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::PageDown,
            KeyCode::End,
            KeyCode::Home,
        ] {
            assert_eq!(move_selection(key, 0, 0, 5), 0, "{key:?}");
        }
    }

    #[test]
    fn a_zero_page_still_moves() {
        // page comes from the drawn height; a degenerate terminal must not
        // leave PgDn doing nothing at all.
        assert_eq!(move_selection(KeyCode::PageDown, 0, 10, 0), 1);
    }

    #[test]
    fn search_reaches_the_subtitle_not_just_the_name() {
        // The aor is right there in the row; typing part of it has to work.
        let items = [
            item("[DEV] Channel 01", "sip:1125452e0@sipgate.de"),
            item("[LOCAL] Lab", "sip:7001@192.168.1.10"),
        ];
        assert_eq!(matching("sipgate", &items), ["[DEV] Channel 01"]);
        assert_eq!(matching("7001", &items), ["[LOCAL] Lab"]);
        assert_eq!(matching("channel", &items), ["[DEV] Channel 01"]);
    }

    #[test]
    fn tokens_may_come_from_either_field_in_any_order() {
        // The point of splitting on whitespace: you type what you remember,
        // not what lives in which column.
        let items = [
            item("[DEV] Channel 01", "sip:1125452e0@sipgate.de"),
            item("[LIVE] Channel 01", "sip:9002@example.com"),
        ];
        assert_eq!(matching("dev sipgate", &items), ["[DEV] Channel 01"]);
        assert_eq!(matching("sipgate dev", &items), ["[DEV] Channel 01"]);
        assert_eq!(matching("channel", &items).len(), 2);
    }

    #[test]
    fn matching_is_by_substring_not_subsequence() {
        // Despite what the old name said, this filter was never fuzzy — and it
        // should not become fuzzy by accident: s-i-p-e appears in order in most
        // aors, so a subsequence match over a subtitle would find everything.
        let items = [item("Lab", "sip:7001@example.com")];
        assert!(matching("sipe", &items).is_empty());
        assert!(matching("lb", &items).is_empty());
        assert_eq!(matching("lab", &items), ["Lab"]);
    }

    #[test]
    fn an_empty_query_keeps_everything() {
        let items = [item("A", "x"), item("B", "y")];
        assert_eq!(matching("", &items), ["A", "B"]);
    }

    #[test]
    fn the_block_letters_want_a_tall_terminal() {
        // Spelled out rather than derived, so a change to the constants has to
        // be a decision about this number and not a silent drift.
        assert_eq!(ROOMY_H, 25);
        assert_eq!(logo_for(LogoMode::Auto, 80, 24, 1), LOGO_SMALL, "24 rows");
        assert_eq!(logo_for(LogoMode::Auto, 80, 25, 1), LOGO, "25 rows");
    }

    #[test]
    fn auto_uses_the_block_letters_when_they_fit() {
        assert_eq!(logo_for(LogoMode::Auto, 80, ROOMY_H, 1), LOGO);
    }

    #[test]
    fn auto_gives_way_on_a_short_terminal() {
        // The list is what the picker is for; the wordmark yields to it.
        assert_eq!(logo_for(LogoMode::Auto, 80, ROOMY_H - 1, 1), LOGO_SMALL);
        assert_eq!(logo_for(LogoMode::Auto, 80, 12, 1), LOGO_SMALL);
    }

    #[test]
    fn auto_gives_way_on_a_narrow_terminal() {
        // Block letters 40 columns wide wrap into nonsense below that.
        assert_eq!(logo_for(LogoMode::Auto, 39, 60, 1), LOGO_SMALL);
        assert_eq!(logo_for(LogoMode::Auto, 40, 60, 1), LOGO);
    }

    #[test]
    fn a_taller_hint_bar_counts_against_the_logo() {
        // A narrow terminal wraps the hints onto more rows, and those rows come
        // out of the same budget.
        assert_eq!(logo_for(LogoMode::Auto, 80, ROOMY_H, 1), LOGO);
        assert_eq!(logo_for(LogoMode::Auto, 80, ROOMY_H, 3), LOGO_SMALL);
    }

    #[test]
    fn the_explicit_modes_ignore_the_terminal() {
        assert_eq!(logo_for(LogoMode::Full, 20, 10, 1), LOGO, "full means full");
        assert_eq!(logo_for(LogoMode::Small, 200, 200, 1), LOGO_SMALL);
        assert!(logo_for(LogoMode::Off, 200, 200, 1).is_empty());
    }
}
