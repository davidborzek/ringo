//! Mouse support shared by every screen: hit regions recorded while rendering,
//! double-click detection, drag-selection with clipboard copy, and the
//! terminal's mouse-capture state.

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    text::Span,
};
use std::{
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// Two presses on the same cell within this window count as a double click.
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);

/// List rows one wheel notch moves.
pub const WHEEL_ROWS: usize = 3;

/// Most text one selection puts on the clipboard.
const MAX_COPY_BYTES: usize = 100_000;

/// The scrollable or selectable lists a click or wheel event can address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListId {
    Calls,
    Logs,
    History,
    Contacts,
    DialHistory,
    Picker,
    FormTabs,
    FormFields,
    Codecs,
    Headers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickAction {
    /// Behave as if this key had been pressed, so context-dependent meanings
    /// stay with the existing key handlers.
    Key(KeyEvent),
    /// Run this command as if typed after `:`.
    Command(&'static str),
    /// Select row `index` of `list`.
    Row { list: ListId, index: usize },
    /// The wheel moves `list`.
    Scroll { list: ListId },
    /// Swallows the click, so a modal does not pass it on to what it covers.
    Inert,
    /// Outside a modal that a click away closes (not confirmations): lies under
    /// the modal, so only clicks that miss it land here.
    Dismiss,
    /// The phone's dial / DTMF line: opens the dial pad.
    OpenDialpad,
}

/// Clickable regions of the last rendered frame.
#[derive(Debug, Default)]
pub struct HitMap(Vec<(Rect, ClickAction)>);

impl HitMap {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn push(&mut self, rect: Rect, action: ClickAction) {
        self.0.push((rect, action));
    }

    /// The action under the cell; the region pushed last wins, because overlays
    /// are drawn after what they cover.
    pub fn hit(&self, col: u16, row: u16) -> Option<&ClickAction> {
        let pos = Position::new(col, row);
        self.0
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, action)| action)
    }

    /// The list the wheel moves at the cell. Looked up apart from `hit`: a row
    /// or hint under the pointer must not stop the wheel from scrolling its list,
    /// while a modal drawn over a list (`Inert`) must.
    pub fn scroll_at(&self, col: u16, row: u16) -> Option<ListId> {
        let pos = Position::new(col, row);
        self.0
            .iter()
            .rev()
            .filter(|(rect, _)| rect.contains(pos))
            .find_map(|(_, action)| match action {
                ClickAction::Scroll { list } => Some(Some(*list)),
                ClickAction::Inert | ClickAction::Dismiss => Some(None),
                _ => None,
            })
            .flatten()
    }
}

#[derive(Debug, Default)]
pub struct DoubleClick {
    last: Option<(Instant, u16, u16)>,
}

impl DoubleClick {
    /// Record a press and say whether it completes a double click. A completed
    /// double click is consumed, so a third press starts over.
    pub fn register(&mut self, col: u16, row: u16, now: Instant) -> bool {
        let is_double = self.last.is_some_and(|(at, last_col, last_row)| {
            (last_col, last_row) == (col, row)
                && now.saturating_duration_since(at) <= DOUBLE_CLICK_WINDOW
        });
        self.last = if is_double {
            None
        } else {
            Some((now, col, row))
        };
        is_double
    }
}

/// Index of the first row to draw so that `focused` is inside a window of
/// `visible` rows. Derived from the focus alone: a list whose focus moves by
/// wheel, click or keys needs no scroll state of its own.
pub fn offset_for_focus(focused: usize, visible: usize) -> usize {
    (focused + 1).saturating_sub(visible.max(1))
}

/// `index` moved `rows` towards the end (`down`) or the start of a list of
/// `len`, stopping at the ends. The wheel does not wrap the way arrow keys do.
pub fn step_clamped(index: usize, len: usize, down: bool, rows: usize) -> usize {
    match (down, len) {
        (_, 0) => 0,
        (true, _) => (index + rows).min(len - 1),
        (false, _) => index.saturating_sub(rows),
    }
}

/// A press or wheel notch resolved against the regions of the last frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pointed {
    /// `col` tells which part of a row was hit; `double` completes a double
    /// click on a row.
    Click {
        action: ClickAction,
        col: u16,
        double: bool,
    },
    Wheel {
        list: ListId,
        down: bool,
    },
}

/// The next event a screen with its own blocking loop acts on.
#[derive(Debug, PartialEq, Eq)]
pub enum Input {
    Key(KeyEvent),
    Pointer(Pointed),
    /// Nothing to act on, but what is on screen changed (resize, selection).
    Redraw,
}

/// Regions, selection, double-click state and the on/off switch for a screen
/// that reads events itself (picker, profile form, its popups), as `App` does
/// for the phone.
#[derive(Debug)]
pub struct Pointer {
    enabled: bool,
    pub hits: HitMap,
    double_click: DoubleClick,
    selector: Selector,
}

impl Pointer {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            hits: HitMap::default(),
            double_click: DoubleClick::default(),
            selector: Selector::default(),
        }
    }

    /// Draw a frame. `render` fills it and records the regions; then the
    /// selection is highlighted, the frame kept for copying, and a finished
    /// selection sent to the clipboard.
    pub(crate) fn draw(
        &mut self,
        terminal: &mut crate::tui::Term,
        render: impl FnOnce(&mut Frame, &mut HitMap),
    ) -> io::Result<()> {
        let Self {
            enabled,
            hits,
            selector,
            ..
        } = self;
        let completed = terminal.draw(|frame| {
            render(frame, hits);
            selector.highlight(frame.buffer_mut());
        })?;
        if *enabled {
            selector.set_frame(completed.buffer.clone());
        }
        selector.flush_copy(terminal.backend_mut())
    }

    /// What a mouse event means; `None` when the mouse is off or the event
    /// changes nothing: a button other than the left one, a move, a click over
    /// nothing that reacts to it.
    pub fn resolve(&mut self, mouse: MouseEvent) -> Option<Input> {
        if !self.enabled {
            return None;
        }
        let (col, row) = (mouse.column, mouse.row);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.selector.press(col, row).then_some(Input::Redraw)
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.selector.drag(col, row).then_some(Input::Redraw)
            }
            MouseEventKind::Up(MouseButton::Left) => match self.selector.release(col, row) {
                Release::Nothing => None,
                Release::Selected { .. } => Some(Input::Redraw),
                Release::Click { col, row } => self.click_at(col, row),
            },
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let down = mouse.kind == MouseEventKind::ScrollDown;
                let cleared = self.selector.clear();
                match self.hits.scroll_at(col, row) {
                    Some(list) => Some(Input::Pointer(Pointed::Wheel { list, down })),
                    None => cleared.then_some(Input::Redraw),
                }
            }
            _ => None,
        }
    }

    fn click_at(&mut self, col: u16, row: u16) -> Option<Input> {
        let action = self.hits.hit(col, row)?.clone();
        let double = matches!(action, ClickAction::Row { .. })
            && self.double_click.register(col, row, Instant::now());
        Some(Input::Pointer(Pointed::Click {
            action,
            col,
            double,
        }))
    }

    /// Block for the next key, resize or effective mouse event. Moves and clicks
    /// on nothing are swallowed here, so they cost the caller no redraw.
    pub fn read(&mut self) -> io::Result<Input> {
        loop {
            match event::read()? {
                Event::Key(key) => {
                    self.selector.clear();
                    return Ok(Input::Key(key));
                }
                Event::Resize(..) => {
                    self.selector.clear();
                    return Ok(Input::Redraw);
                }
                Event::Mouse(mouse) => {
                    if let Some(input) = self.resolve(mouse) {
                        return Ok(input);
                    }
                }
                _ => {}
            }
        }
    }

    /// For screens whose only mouse targets are keys: the key pressed, or the
    /// key a click stands for (a click away from the popup is Esc). `None` for
    /// everything else.
    pub fn read_key(&mut self) -> io::Result<Option<KeyEvent>> {
        Ok(match self.read()? {
            Input::Key(key) => Some(key),
            Input::Pointer(Pointed::Click {
                action: ClickAction::Key(key),
                ..
            }) => Some(key),
            Input::Pointer(Pointed::Click {
                action: ClickAction::Dismiss,
                ..
            }) => Some(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            _ => None,
        })
    }
}

/// A linear run of cells from `anchor` to `cursor`, both included, as a terminal
/// selects: along the rows, wrapping at the line ends. Either end may come
/// first. Cells are `(col, row)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: (u16, u16),
    pub cursor: (u16, u16),
}

impl Selection {
    /// The first and the last cell in reading order.
    fn ordered(self) -> ((u16, u16), (u16, u16)) {
        let reading_order = |(col, row): (u16, u16)| (row, col);
        if reading_order(self.anchor) <= reading_order(self.cursor) {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    /// The selected cells per row as `(row, first_col, last_col)`, clamped to
    /// the buffer: the pointer can be outside a frame kept from before a resize.
    /// An edge that falls on the second half of a wide glyph snaps to the
    /// glyph's lead cell, so extraction and highlight cover what the terminal
    /// really shows.
    fn rows(self, buf: &Buffer) -> Vec<(u16, u16, u16)> {
        let area = buf.area;
        if area.is_empty() {
            return Vec::new();
        }
        let clamp_col = |col: u16| col.clamp(area.left(), area.right() - 1);
        let clamp_row = |row: u16| row.clamp(area.top(), area.bottom() - 1);
        let ((start_col, start_row), (end_col, end_row)) = self.ordered();
        let (start_row, end_row) = (clamp_row(start_row), clamp_row(end_row));
        let (start_col, end_col) = (
            glyph_start(buf, clamp_col(start_col), start_row),
            glyph_start(buf, clamp_col(end_col), end_row),
        );
        (start_row..=end_row)
            .map(|row| {
                let first = if row == start_row {
                    start_col
                } else {
                    area.left()
                };
                let last = if row == end_row {
                    end_col
                } else {
                    area.right() - 1
                };
                (row, first, last)
            })
            .collect()
    }
}

/// The first cell of the glyph at `(col, row)`. A wide glyph is stored in its
/// lead cell; the terminal skips the blank cells after it, so a selection edge
/// on one of them snaps back to the lead cell.
fn glyph_start(buf: &Buffer, col: u16, row: u16) -> u16 {
    let mut col = col;
    while col > buf.area.left() && Span::raw(buf[(col - 1, row)].symbol()).width() > 1 {
        col -= 1;
    }
    col
}

/// The text under a selection: per row the cells' symbols without trailing
/// spaces, rows joined by newlines. A wide glyph is copied whole whichever of
/// its cells an edge falls on.
pub fn selected_text(buf: &Buffer, selection: Selection) -> String {
    let rows: Vec<String> = selection
        .rows(buf)
        .into_iter()
        .map(|(row, first, last)| {
            let mut line = String::new();
            let mut col = first;
            while col <= last {
                let symbol = buf[(col, row)].symbol();
                line.push_str(symbol);
                // A wide glyph is followed by cells ratatui leaves blank.
                col += Span::raw(symbol).width().max(1) as u16;
            }
            line.trim_end_matches(' ').to_string()
        })
        .collect();
    rows.join("\n")
}

/// Draw the selection by reversing its cells. Toggled rather than set, so an
/// already reversed cell (a selected list row) still differs from its neighbours.
/// Glyph-aware like [`selected_text`], so a wide glyph's visible lead cell is
/// reversed even when the selection starts on its second half.
pub fn highlight(buf: &mut Buffer, selection: Selection) {
    for (row, first, last) in selection.rows(buf) {
        for col in first..=last {
            buf[(col, row)].modifier.toggle(Modifier::REVERSED);
        }
    }
}

/// What releasing the left button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    /// No press went with it.
    Nothing,
    /// Pressed and released on one cell without dragging: a click on it.
    Click { col: u16, row: u16 },
    /// A selection was made; `chars` of its text were queued for the clipboard
    /// (none when it held only blanks).
    Selected { chars: usize },
}

/// Drag-selection over the last rendered frame. The terminal cannot select
/// while mouse capture is on, so the screen does it itself: a press records the
/// anchor, dragging extends the selection, releasing copies its text.
#[derive(Debug, Default)]
pub struct Selector {
    /// The last frame as drawn, which the text is read from.
    frame: Option<Buffer>,
    /// Where the left button went down, while it is held.
    press: Option<(u16, u16)>,
    /// Being dragged, or kept highlighted after the release.
    selection: Option<Selection>,
    /// Text for the next [`Selector::flush_copy`].
    copy: Option<String>,
}

impl Selector {
    pub fn set_frame(&mut self, frame: Buffer) {
        self.frame = Some(frame);
    }

    /// Reverse the selected cells of the frame about to be flushed.
    pub fn highlight(&self, buf: &mut Buffer) {
        if let Some(selection) = self.selection {
            highlight(buf, selection);
        }
    }

    /// Drop the selection and a press in progress. Whether a selection was
    /// showing, i.e. whether the screen needs drawing again.
    pub fn clear(&mut self) -> bool {
        self.press = None;
        self.selection.take().is_some()
    }

    /// Left button down: a new gesture, and the end of a kept selection.
    /// Whether that selection was showing.
    pub fn press(&mut self, col: u16, row: u16) -> bool {
        let cleared = self.clear();
        self.press = Some((col, row));
        cleared
    }

    /// Left button moved while down. Whether the selection changed: moving
    /// within one cell changes nothing and must not cost a redraw.
    pub fn drag(&mut self, col: u16, row: u16) -> bool {
        let Some(anchor) = self.press else {
            return false;
        };
        let cursor = (col, row);
        let next =
            (cursor != anchor || self.selection.is_some()).then_some(Selection { anchor, cursor });
        let changed = next != self.selection;
        self.selection = next;
        changed
    }

    /// Left button up. A release away from the press selects even if the
    /// terminal sent no drag events in between.
    pub fn release(&mut self, col: u16, row: u16) -> Release {
        let Some(anchor) = self.press.take() else {
            return Release::Nothing;
        };
        let cursor = (col, row);
        if cursor == anchor && self.selection.is_none() {
            return Release::Click { col, row };
        }
        let selection = Selection { anchor, cursor };
        self.selection = Some(selection);
        let mut text = self
            .frame
            .as_ref()
            .map(|frame| selected_text(frame, selection))
            .unwrap_or_default();
        if text.trim().is_empty() {
            return Release::Selected { chars: 0 };
        }
        if text.len() > MAX_COPY_BYTES {
            let mut end = MAX_COPY_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        let chars = text.chars().count();
        self.copy = Some(text);
        Release::Selected { chars }
    }

    /// Send a queued selection to the clipboard. `w` must be the terminal's own
    /// writer: fd 1 is the log while the phone runs.
    pub fn flush_copy<W: Write>(&mut self, w: &mut W) -> io::Result<()> {
        match self.copy.take() {
            Some(text) => {
                w.write_all(&osc52(&text, in_tmux()))?;
                w.flush()
            }
            None => Ok(()),
        }
    }
}

fn in_tmux() -> bool {
    std::env::var_os("TMUX").is_some_and(|tmux| !tmux.is_empty())
}

/// The OSC 52 sequence that sets the system clipboard to `text`. tmux drops
/// sequences it does not know unless they are wrapped for passthrough, with
/// every ESC inside doubled.
pub fn osc52(text: &str, in_tmux: bool) -> Vec<u8> {
    let osc = format!("\x1b]52;c;{}\x07", base64(text.as_bytes()));
    if in_tmux {
        format!("\x1bPtmux;{}\x1b\\", osc.replace('\x1b', "\x1b\x1b")).into_bytes()
    } else {
        osc.into_bytes()
    }
}

/// Standard base64 with padding, the encoding OSC 52 carries.
pub fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let byte = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let bits = byte(0) << 16 | byte(1) << 8 | byte(2);
        let sextet = |shift: u32| ALPHABET[(bits >> shift) as usize & 63] as char;
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
    out
}

/// What the terminal currently has, for the panic hook and for code that
/// leaves the alternate screen and has to put capture back afterwards.
static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn capture_active() -> bool {
    CAPTURE_ACTIVE.load(Ordering::SeqCst)
}

/// Turn mouse capture on or off. Writes nothing when it already is in that
/// state, so a session with the mouse off never emits a capture sequence.
pub fn set_capture<W: Write>(w: &mut W, on: bool) -> io::Result<()> {
    if capture_active() == on {
        return Ok(());
    }
    write_capture(w, on)?;
    CAPTURE_ACTIVE.store(on, Ordering::SeqCst);
    Ok(())
}

fn write_capture<W: Write>(w: &mut W, on: bool) -> io::Result<()> {
    if on {
        crossterm::execute!(w, EnableMouseCapture)
    } else {
        crossterm::execute!(w, DisableMouseCapture)
    }
}

/// Make a panic hand the terminal back before the previous hook prints, so the
/// message is readable and the shell gets no stray mouse reports. Only the
/// thread that installs it is the TUI thread: panics that baresip callbacks
/// catch on the RE thread must not tear the screen down under a running phone.
pub fn install_panic_hook() {
    use std::sync::Once;
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let tui_thread = std::thread::current().id();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if std::thread::current().id() == tui_thread && capture_active() {
                restore_terminal_from_tty();
            }
            previous(info);
        }));
    });
}

/// Hand the terminal back if a full-screen UI is still up: capture off, and
/// raw mode and the alternate screen left (raw mode is what marks the screen as
/// held). Safe to call when nothing is held or after the screen was already
/// left: it then writes nothing.
pub fn restore_terminal_to<W: Write>(w: &mut W) {
    let _ = set_capture(w, false);
    if crossterm::terminal::is_raw_mode_enabled().unwrap_or(false) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(w, crossterm::terminal::LeaveAlternateScreen);
    }
}

/// [`restore_terminal_to`] on the controlling terminal: fd 1 may point at the
/// profile log, which must not receive escape sequences.
pub fn restore_terminal() {
    match std::fs::OpenOptions::new().write(true).open("/dev/tty") {
        Ok(mut tty) => restore_terminal_to(&mut tty),
        Err(_) => restore_terminal_to(&mut io::stdout()),
    }
}

/// Fd 1/2 point at the profile log while the TUI runs, so go to the controlling
/// terminal directly.
fn restore_terminal_from_tty() {
    use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
    if let Ok(mut tty) = std::fs::OpenOptions::new().write(true).open("/dev/tty") {
        let _ = crossterm::execute!(tty, DisableMouseCapture, LeaveAlternateScreen);
    }
    let _ = disable_raw_mode();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn key(c: char) -> ClickAction {
        ClickAction::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    #[test]
    fn hit_returns_the_topmost_region() {
        let mut hits = HitMap::default();
        hits.push(Rect::new(0, 0, 10, 10), key('a'));
        hits.push(Rect::new(2, 2, 3, 3), key('b'));
        assert_eq!(hits.hit(3, 3), Some(&key('b')));
        assert_eq!(hits.hit(0, 0), Some(&key('a')));
    }

    #[test]
    fn hit_outside_every_region_is_none() {
        let mut hits = HitMap::default();
        hits.push(Rect::new(2, 2, 3, 3), key('a'));
        assert_eq!(hits.hit(5, 3), None, "right edge is exclusive");
        assert_eq!(hits.hit(1, 2), None);
        hits.clear();
        assert_eq!(hits.hit(3, 3), None);
    }

    #[test]
    fn scroll_at_sees_through_rows_and_keys_above_the_list() {
        let mut hits = HitMap::default();
        hits.push(
            Rect::new(0, 0, 10, 10),
            ClickAction::Scroll { list: ListId::Logs },
        );
        hits.push(
            Rect::new(0, 2, 10, 1),
            ClickAction::Row {
                list: ListId::Logs,
                index: 0,
            },
        );
        hits.push(Rect::new(0, 5, 4, 1), key('a'));
        assert_eq!(hits.scroll_at(1, 2), Some(ListId::Logs));
        assert_eq!(hits.scroll_at(1, 5), Some(ListId::Logs));
        assert_eq!(hits.scroll_at(11, 5), None);
    }

    #[test]
    fn scroll_at_stops_at_a_modal_over_the_list() {
        let mut hits = HitMap::default();
        hits.push(
            Rect::new(0, 0, 10, 10),
            ClickAction::Scroll { list: ListId::Logs },
        );
        hits.push(Rect::new(2, 2, 3, 3), ClickAction::Inert);
        assert_eq!(hits.scroll_at(3, 3), None);
        assert_eq!(hits.scroll_at(0, 0), Some(ListId::Logs));
    }

    #[test]
    fn double_click_on_the_same_cell_within_the_window() {
        let t0 = Instant::now();
        let mut dc = DoubleClick::default();
        assert!(!dc.register(4, 7, t0));
        assert!(dc.register(4, 7, t0 + Duration::from_millis(399)));
        assert!(
            !dc.register(4, 7, t0 + Duration::from_millis(500)),
            "a double click is consumed"
        );
    }

    #[test]
    fn no_double_click_on_another_cell_or_after_the_window() {
        let t0 = Instant::now();
        let mut dc = DoubleClick::default();
        assert!(!dc.register(4, 7, t0));
        assert!(!dc.register(4, 8, t0 + Duration::from_millis(100)));
        assert!(!dc.register(4, 8, t0 + Duration::from_millis(600)));
    }

    fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    fn down(col: u16, row: u16) -> MouseEvent {
        mouse(MouseEventKind::Down(MouseButton::Left), col, row)
    }

    fn up(col: u16, row: u16) -> MouseEvent {
        mouse(MouseEventKind::Up(MouseButton::Left), col, row)
    }

    fn drag(col: u16, row: u16) -> MouseEvent {
        mouse(MouseEventKind::Drag(MouseButton::Left), col, row)
    }

    fn row_pointer(enabled: bool) -> Pointer {
        let mut pointer = Pointer::new(enabled);
        pointer.hits.push(
            Rect::new(0, 0, 10, 1),
            ClickAction::Row {
                list: ListId::Picker,
                index: 4,
            },
        );
        pointer.hits.push(
            Rect::new(0, 1, 10, 1),
            ClickAction::Scroll {
                list: ListId::Picker,
            },
        );
        pointer
    }

    /// Press and release on one cell, as a click arrives.
    fn click(pointer: &mut Pointer, col: u16, row: u16) -> Option<Input> {
        assert_eq!(pointer.resolve(down(col, row)), None);
        pointer.resolve(up(col, row))
    }

    #[test]
    fn pointer_ignores_everything_while_off() {
        let mut pointer = row_pointer(false);
        for event in [
            down(1, 0),
            drag(5, 0),
            up(5, 0),
            mouse(MouseEventKind::ScrollDown, 1, 1),
        ] {
            assert_eq!(pointer.resolve(event), None);
        }
        let mut out = Vec::new();
        pointer.selector.flush_copy(&mut out).unwrap();
        assert!(out.is_empty(), "nothing is ever copied while off");
    }

    #[test]
    fn pointer_acts_on_release_not_on_press() {
        let mut pointer = row_pointer(true);
        assert_eq!(
            pointer.resolve(down(3, 0)),
            None,
            "a press alone does nothing"
        );
        assert_eq!(
            pointer.resolve(up(3, 0)),
            Some(Input::Pointer(Pointed::Click {
                action: ClickAction::Row {
                    list: ListId::Picker,
                    index: 4
                },
                col: 3,
                double: false
            }))
        );
        assert_eq!(pointer.resolve(up(3, 0)), None, "a release without a press");
    }

    #[test]
    fn pointer_flags_the_second_click_on_a_row_as_double() {
        let mut pointer = row_pointer(true);
        let row = ClickAction::Row {
            list: ListId::Picker,
            index: 4,
        };
        assert_eq!(
            click(&mut pointer, 3, 0),
            Some(Input::Pointer(Pointed::Click {
                action: row.clone(),
                col: 3,
                double: false
            }))
        );
        assert_eq!(
            click(&mut pointer, 3, 0),
            Some(Input::Pointer(Pointed::Click {
                action: row,
                col: 3,
                double: true
            }))
        );
        assert_eq!(click(&mut pointer, 30, 9), None, "a click on nothing");
    }

    #[test]
    fn pointer_resolves_the_wheel_to_the_list_under_it() {
        let mut pointer = row_pointer(true);
        let wheel = |kind| mouse(kind, 2, 1);
        assert_eq!(
            pointer.resolve(wheel(MouseEventKind::ScrollUp)),
            Some(Input::Pointer(Pointed::Wheel {
                list: ListId::Picker,
                down: false
            }))
        );
        assert_eq!(
            pointer.resolve(wheel(MouseEventKind::ScrollDown)),
            Some(Input::Pointer(Pointed::Wheel {
                list: ListId::Picker,
                down: true
            }))
        );
        assert_eq!(pointer.resolve(wheel(MouseEventKind::Moved)), None);
    }

    #[test]
    fn a_drag_selects_instead_of_clicking() {
        let mut pointer = row_pointer(true);
        assert_eq!(pointer.resolve(down(1, 0)), None);
        assert_eq!(pointer.resolve(drag(1, 0)), None, "same cell: no change");
        assert_eq!(pointer.resolve(drag(4, 0)), Some(Input::Redraw));
        assert_eq!(pointer.resolve(drag(4, 0)), None, "same range: no redraw");
        assert_eq!(pointer.resolve(up(4, 0)), Some(Input::Redraw));
        // The kept highlight goes with the next press.
        assert_eq!(pointer.resolve(down(7, 0)), Some(Input::Redraw));
        assert_eq!(
            pointer
                .resolve(up(7, 0))
                .map(|i| matches!(i, Input::Pointer(_))),
            Some(true)
        );
    }

    #[test]
    fn drag_events_without_a_press_do_nothing() {
        let mut selector = Selector::default();
        assert!(!selector.drag(3, 3));
        assert_eq!(selector.release(3, 3), Release::Nothing);
    }

    #[test]
    fn a_release_away_from_the_press_selects_without_drag_events() {
        let mut selector = Selector::default();
        selector.set_frame(screen(&["hello world"]));
        selector.press(0, 0);
        assert_eq!(selector.release(4, 0), Release::Selected { chars: 5 });
    }

    #[test]
    fn the_wheel_and_keys_drop_the_selection() {
        let mut pointer = row_pointer(true);
        pointer.resolve(down(1, 0));
        pointer.resolve(drag(4, 0));
        pointer.resolve(up(4, 0));
        assert_eq!(
            pointer.resolve(mouse(MouseEventKind::ScrollDown, 20, 20)),
            Some(Input::Redraw),
            "the wheel over nothing still clears the highlight"
        );
        assert!(!pointer.selector.clear());
    }

    /// A buffer holding `lines`, as a `TestBackend` frame would.
    fn screen(lines: &[&str]) -> Buffer {
        let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let mut buf = Buffer::empty(Rect::new(0, 0, width as u16 + 4, lines.len() as u16));
        for (row, line) in lines.iter().enumerate() {
            buf.set_string(0, row as u16, line, ratatui::style::Style::default());
        }
        buf
    }

    fn select(anchor: (u16, u16), cursor: (u16, u16)) -> Selection {
        Selection { anchor, cursor }
    }

    #[test]
    fn selected_text_spans_rows_and_trims_trailing_spaces() {
        let buf = screen(&["alpha   beta", "gamma", "delta eps"]);
        // From the "b" of row 0 through "del" of row 2: row 1 whole, in between.
        assert_eq!(
            selected_text(&buf, select((8, 0), (2, 2))),
            "beta\ngamma\ndel"
        );
        assert_eq!(selected_text(&buf, select((0, 0), (6, 0))), "alpha");
    }

    #[test]
    fn selected_text_is_the_same_whichever_end_was_pressed_first() {
        let buf = screen(&["alpha   beta", "gamma", "delta eps"]);
        assert_eq!(
            selected_text(&buf, select((2, 2), (8, 0))),
            selected_text(&buf, select((8, 0), (2, 2)))
        );
        assert_eq!(selected_text(&buf, select((4, 1), (1, 1))), "amma");
    }

    #[test]
    fn selected_text_keeps_a_wide_glyph_once() {
        let buf = screen(&["a日本b"]);
        assert_eq!(selected_text(&buf, select((0, 0), (5, 0))), "a日本b");
        assert_eq!(selected_text(&buf, select((1, 0), (2, 0))), "日");
    }

    #[test]
    fn selected_text_snaps_a_start_on_a_wide_glyphs_second_half_to_its_lead() {
        // 日 spans the cells 1 and 2, 本 the cells 3 and 4; the cells after the
        // leads are blanks the terminal skips.
        let buf = screen(&["a日本b"]);
        assert_eq!(selected_text(&buf, select((1, 0), (5, 0))), "日本b");
        assert_eq!(selected_text(&buf, select((2, 0), (5, 0))), "日本b");
        assert_eq!(selected_text(&buf, select((4, 0), (5, 0))), "本b");
        assert_eq!(selected_text(&buf, select((2, 0), (2, 0))), "日");
    }

    #[test]
    fn selected_text_keeps_a_glyph_whose_second_half_ends_the_selection() {
        let buf = screen(&["a日本b"]);
        assert_eq!(selected_text(&buf, select((0, 0), (2, 0))), "a日");
        assert_eq!(
            selected_text(&buf, select((0, 0), (3, 0))),
            "a日本",
            "ending on the lead cell includes the glyph"
        );
        assert_eq!(selected_text(&buf, select((0, 0), (4, 0))), "a日本");
        assert_eq!(selected_text(&buf, select((3, 0), (4, 0))), "本");
    }

    #[test]
    fn highlight_on_a_wide_glyphs_second_half_reverses_its_lead_cell() {
        let mut buf = screen(&["a日本b"]);
        highlight(&mut buf, select((2, 0), (5, 0)));
        let reversed = |col: u16, row: u16| buf[(col, row)].modifier.contains(Modifier::REVERSED);
        assert!(!reversed(0, 0), "the glyph before the start is untouched");
        assert!(reversed(1, 0), "日's lead cell is reversed");
        assert!(reversed(3, 0), "本's lead cell is reversed");
        assert!(reversed(5, 0), "the last cell is reversed");
    }

    #[test]
    fn a_selection_outside_a_smaller_frame_is_clamped() {
        let buf = screen(&["abc"]);
        assert_eq!(selected_text(&buf, select((1, 0), (90, 40))), "bc");
        assert_eq!(
            selected_text(&Buffer::empty(Rect::ZERO), select((0, 0), (3, 3))),
            ""
        );
    }

    #[test]
    fn highlight_reverses_exactly_the_selected_cells() {
        let mut buf = screen(&["abcdef", "ghijkl"]);
        buf[(1, 1)].modifier.insert(Modifier::REVERSED);
        highlight(&mut buf, select((4, 0), (1, 1)));
        let reversed = |col: u16, row: u16| buf[(col, row)].modifier.contains(Modifier::REVERSED);
        assert!(!reversed(3, 0));
        assert!(
            reversed(4, 0) && reversed(9, 0),
            "to the end of the first row"
        );
        assert!(reversed(0, 1));
        assert!(!reversed(1, 1), "an already reversed cell is toggled off");
        assert!(!reversed(2, 1));
    }

    #[test]
    fn releasing_a_selection_queues_its_text_once() {
        let mut selector = Selector::default();
        selector.set_frame(screen(&["hello world", "second"]));
        selector.press(6, 0);
        selector.drag(2, 1);
        assert_eq!(selector.release(2, 1), Release::Selected { chars: 9 });

        let mut out = Vec::new();
        selector.flush_copy(&mut out).unwrap();
        assert_eq!(out, osc52("world\nsec", in_tmux()));
        out.clear();
        selector.flush_copy(&mut out).unwrap();
        assert!(out.is_empty(), "sent only once");
    }

    #[test]
    fn a_selection_of_blanks_copies_nothing() {
        let mut selector = Selector::default();
        selector.set_frame(screen(&["ab        cd"]));
        selector.press(3, 0);
        assert_eq!(selector.release(7, 0), Release::Selected { chars: 0 });
        let mut out = Vec::new();
        selector.flush_copy(&mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn a_selection_without_a_frame_copies_nothing() {
        let mut selector = Selector::default();
        selector.press(0, 0);
        assert_eq!(selector.release(5, 0), Release::Selected { chars: 0 });
    }

    #[test]
    fn copied_text_is_capped_on_a_character_boundary() {
        let mut selector = Selector::default();
        let line = "é".repeat(60_000);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60_000, 1));
        buf.set_string(0, 0, &line, ratatui::style::Style::default());
        selector.set_frame(buf);
        selector.press(0, 0);
        let Release::Selected { chars } = selector.release(59_999, 0) else {
            panic!("expected a selection");
        };
        assert_eq!(chars, MAX_COPY_BYTES / 2);
    }

    #[test]
    fn base64_matches_the_rfc_4648_vectors() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), encoded, "{plain:?}");
        }
        assert_eq!(base64(&[0xfb, 0xff, 0xfe]), "+//+", "the last two symbols");
    }

    #[test]
    fn osc52_sets_the_clipboard_and_is_wrapped_for_tmux() {
        assert_eq!(osc52("hi", false), b"\x1b]52;c;aGk=\x07");
        assert_eq!(osc52("hi", true), b"\x1bPtmux;\x1b\x1b]52;c;aGk=\x07\x1b\\");
    }

    #[test]
    fn step_clamped_stops_at_both_ends() {
        assert_eq!(step_clamped(1, 10, true, 3), 4);
        assert_eq!(step_clamped(8, 10, true, 3), 9);
        assert_eq!(step_clamped(1, 10, false, 3), 0);
        assert_eq!(step_clamped(5, 10, false, 3), 2);
        assert_eq!(step_clamped(0, 0, true, 3), 0);
    }

    #[test]
    fn offset_for_focus_keeps_the_focused_row_in_the_window() {
        assert_eq!(offset_for_focus(0, 5), 0);
        assert_eq!(offset_for_focus(4, 5), 0);
        assert_eq!(offset_for_focus(5, 5), 1);
        assert_eq!(offset_for_focus(9, 5), 5);
        assert_eq!(offset_for_focus(3, 0), 3, "a zero window counts as one row");
    }

    // CAPTURE_ACTIVE is process-global.
    static CAPTURE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn capture_writes_only_on_a_change() {
        let _guard = CAPTURE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        set_capture(&mut out, false).unwrap();
        assert!(out.is_empty(), "off while already off emits nothing");

        set_capture(&mut out, true).unwrap();
        assert!(capture_active());
        let enabled = out.len();
        assert!(enabled > 0);

        set_capture(&mut out, true).unwrap();
        assert_eq!(out.len(), enabled, "on while already on emits nothing");

        set_capture(&mut out, false).unwrap();
        assert!(!capture_active());
        assert!(out.len() > enabled);
    }

    #[test]
    fn restore_terminal_to_switches_capture_off_once() {
        // Shares CAPTURE_ACTIVE with the test above, so it must not run beside it.
        let _guard = CAPTURE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut setup = Vec::new();
        set_capture(&mut setup, true).unwrap();

        let mut out = Vec::new();
        restore_terminal_to(&mut out);
        assert!(!capture_active());
        assert!(!out.is_empty(), "capture is switched off");

        let written = out.len();
        restore_terminal_to(&mut out);
        assert_eq!(out.len(), written, "a second restore writes nothing");
    }
}
