use crate::{
    config::Hook, config::Theme, contacts::Contact, header::HeaderContext, header::HeaderTemplate,
    phone::Phone, profile::Profile,
};
use std::{collections::VecDeque, path::PathBuf, time::Instant};

// ─── State types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum RegStatus {
    Unknown,
    Registering,
    Ok,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallDirection {
    Outgoing,
    Incoming,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallState {
    Ringing,
    Established,
    OnHold,
}

#[derive(Debug, Clone)]
pub struct Call {
    pub id: String,
    pub direction: CallDirection,
    pub peer: String,
    pub peer_display_name: Option<String>,
    pub state: CallState,
    pub started_at: Option<Instant>,
}

/// A snapshot of the most recently closed call, retained after it leaves
/// `calls` so a status poller can see how (and why) the last call ended.
#[derive(Debug, Clone)]
pub struct LastCall {
    pub peer: String,
    pub direction: String, // "outgoing" | "incoming"
    pub reason: String,
    pub error: bool,
    pub duration_secs: u64,
    pub answered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,        // Default — single keys are shortcuts
    Dial,          // Typing into dial input
    HistoryNav,    // Up/Down through full history
    HistorySearch, // Ctrl+R fuzzy popup
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CallHistoryEntry {
    pub ts: String,
    pub dir: String, // "outgoing" | "incoming"
    pub peer: String,
    pub duration: String,   // "HH:MM:SS" | "missed" | "no answer"
    pub duration_secs: u64, // 0 for missed/no answer
}

#[derive(Debug, Default, PartialEq)]
pub enum TransferMode {
    #[default]
    None,
    BlindInput(String),    // 't' pressed, typing URI
    AttendedInput(String), // 'T' pressed, typing URI
    AttendedPending,       // atransferstart sent, waiting for X or Esc
}

// ─── Sub-structs ──────────────────────────────────────────────────────────────

pub struct DialState {
    pub input: String, // current dial input
    pub cursor: usize, // byte-index cursor within `input`
    pub dtmf: String,  // digits sent during active call (display only)
    pub draft: String, // saved input when entering history mode
    pub history: VecDeque<String>,
    pub mode: InputMode,
    pub nav_idx: usize,  // index for HistoryNav mode
    pub query: String,   // filter query for HistorySearch mode
    pub selected: usize, // selected entry index in HistorySearch mode
}

pub struct MwiState {
    pub waiting: bool,
    pub new_messages: u32,
}

/// Pending call-history deletion awaiting y/n confirmation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HistoryDelete {
    /// The currently selected entry.
    One,
    /// The whole history.
    All,
}

pub struct CallHistoryState {
    pub path: Option<PathBuf>,
    pub entries: Vec<CallHistoryEntry>,
    pub show: bool,
    pub selected: usize,
    pub search_query: String,
    pub search_mode: bool,
    /// Set while a `d`/`D` deletion is waiting for confirmation.
    pub delete_confirm: Option<HistoryDelete>,
}

impl CallHistoryState {
    /// Indices into `entries` that match the current search query.
    pub fn filtered_indices(&self, contacts: &[Contact]) -> Vec<usize> {
        let q = self.search_query.to_lowercase();
        if q.is_empty() {
            return (0..self.entries.len()).collect();
        }
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if e.peer.to_lowercase().contains(&q) {
                    return true;
                }
                if let Some(name) = crate::contacts::resolve_name(contacts, &e.peer) {
                    return name.to_lowercase().contains(&q);
                }
                false
            })
            .map(|(i, _)| i)
            .collect()
    }
}

pub struct LogState {
    /// On-disk log file backing the Logs modal.
    pub path: Option<PathBuf>,
    /// Sanitized lines read from `path`, refreshed while the modal is open.
    pub lines: Vec<String>,
    /// Whether the Logs modal is open.
    pub show: bool,
    /// Scroll offset in display rows, counted back from the bottom (0 = follow).
    pub scroll: usize,
    /// Case-insensitive substring filter (grep-style); empty shows all lines.
    pub search_query: String,
    /// Whether the `/` search input is currently capturing keys.
    pub search_mode: bool,
    /// Soft-wrap long lines instead of truncating them.
    pub wrap: bool,
    /// Last known visible height (set during render, used to clamp scroll).
    pub visible_height: usize,
    /// Total display rows of the current (filtered, possibly wrapped) content,
    /// set during render and used to clamp scrolling.
    pub content_rows: usize,
}

pub struct CommandState {
    pub active: bool,
    pub input: String,
    pub error: Option<String>,
    /// Prefix typed before Tab was first pressed (for cycling through matches).
    pub tab_prefix: Option<String>,
    pub tab_index: usize,
}

/// Where a contact picker selection should be applied.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ContactPickerTarget {
    #[default]
    Dial,
    Transfer,
}

pub struct ContactsState {
    pub show: bool,
    pub selected: usize,
    pub search_query: String,
    pub search_mode: bool,
    pub form: ContactFormState,
    /// Contact index pending deletion (waiting for y/n confirmation).
    pub delete_confirm: Option<usize>,
    /// Where the selected number should go when Enter is pressed.
    pub target: ContactPickerTarget,
}

#[derive(Debug, Default, PartialEq)]
pub enum ContactFormMode {
    #[default]
    None,
    Add,
    Edit(usize), // index into contacts vec
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ContactFormField {
    #[default]
    Name,
    Numbers,
}

pub struct ContactFormState {
    pub mode: ContactFormMode,
    pub field: ContactFormField,
    pub name: String,
    pub numbers: String, // comma-separated
    pub cursor: usize,   // byte cursor in active field
}

// ─── App ──────────────────────────────────────────────────────────────────────

pub struct App {
    pub profile_name: String,
    pub account_aor: String,
    pub reg_status: RegStatus,
    pub calls: Vec<Call>,
    pub selected_call: usize,
    pub muted: bool,
    /// Live media stats for the active (selected) call, polled ~1 Hz. baresip
    /// reports stats for its current call, which `switch_line` keeps in sync
    /// with `selected_call`. `None` when no call is established.
    pub media: Option<crate::event::MediaStats>,
    /// Negotiated audio codec for the active (selected) call, polled with `media`.
    pub codec: Option<crate::event::CodecInfo>,
    pub notify_enabled: bool,
    pub transfer_mode: TransferMode,
    pub tick: u64,
    pub dial: DialState,
    pub mwi: MwiState,
    pub call_history: CallHistoryState,
    pub log: LogState,
    pub last_call_reason: Option<String>,
    /// Richer snapshot of the last closed call, exposed via the remote `status`
    /// command (set for every close, not just errors).
    pub last_call: Option<LastCall>,
    pub command: CommandState,
    pub(crate) phone: Box<dyn Phone>,
    pub quit: bool,
    pub quit_confirm: bool,
    /// Whether the "switch profile" (back to picker) confirm popup is open.
    pub switch_confirm: bool,
    /// Whether the Help modal is open.
    pub help_show: bool,
    /// Which button is highlighted in the active confirm popup (`true` = the
    /// destructive action). Reset to `false` (safe) each time a popup opens.
    pub confirm_yes: bool,
    pub switch_to: bool,
    pub edit_profile: bool,
    pub theme: Theme,
    pub hooks: Vec<Hook>,
    pub profile: Profile,
    pub contacts: Vec<Contact>,
    pub contacts_state: ContactsState,
    /// Custom SIP headers configured for the active profile. Dynamic
    /// templates (e.g. containing `$uuid`) are re-rendered per call by
    /// [`Self::dial`].
    pub custom_headers: Vec<(String, HeaderTemplate)>,
}

impl App {
    pub fn new(
        profile_name: String,
        account_aor: String,
        log_path: Option<PathBuf>,
        call_history_path: Option<PathBuf>,
        notify_enabled: bool,
        phone: Box<dyn Phone>,
        theme: Theme,
        hooks: Vec<Hook>,
        profile: Profile,
        contacts: Vec<Contact>,
        custom_headers: Vec<(String, HeaderTemplate)>,
    ) -> Self {
        Self {
            profile_name,
            account_aor,
            reg_status: RegStatus::Unknown,
            calls: Vec::new(),
            selected_call: 0,
            muted: false,
            media: None,
            codec: None,
            notify_enabled,
            transfer_mode: TransferMode::None,
            tick: 0,
            phone,
            quit: false,
            quit_confirm: false,
            switch_confirm: false,
            help_show: false,
            confirm_yes: false,
            switch_to: false,
            dial: DialState {
                input: String::new(),
                cursor: 0,
                dtmf: String::new(),
                draft: String::new(),
                history: crate::history::load(),
                mode: InputMode::Normal,
                nav_idx: 0,
                query: String::new(),
                selected: 0,
            },
            mwi: MwiState {
                waiting: false,
                new_messages: 0,
            },
            call_history: CallHistoryState {
                path: call_history_path,
                entries: Vec::new(),
                show: false,
                selected: 0,
                search_query: String::new(),
                search_mode: false,
                delete_confirm: None,
            },
            log: LogState {
                path: log_path,
                lines: Vec::new(),
                show: false,
                scroll: 0,
                search_query: String::new(),
                search_mode: false,
                wrap: false,
                visible_height: 0,
                content_rows: 0,
            },
            last_call_reason: None,
            last_call: None,
            command: CommandState {
                active: false,
                input: String::new(),
                error: None,
                tab_prefix: None,
                tab_index: 0,
            },
            edit_profile: false,
            theme,
            hooks,
            profile,
            contacts,
            contacts_state: ContactsState {
                show: false,
                selected: 0,
                search_query: String::new(),
                search_mode: false,
                delete_confirm: None,
                target: ContactPickerTarget::Dial,
                form: ContactFormState {
                    mode: ContactFormMode::None,
                    field: ContactFormField::Name,
                    name: String::new(),
                    numbers: String::new(),
                    cursor: 0,
                },
            },
            custom_headers,
        }
    }

    /// Place an outbound call, re-rendering dynamic custom headers so each
    /// call gets fresh placeholder values.
    pub fn dial(&mut self, target: &str) {
        self.refresh_dynamic_headers();
        self.phone.dial(target);
    }

    fn refresh_dynamic_headers(&self) {
        use std::collections::HashSet;

        let ctx = HeaderContext::for_call();
        // `uarmheader` removes *all* headers with a given name, so once any
        // template for a key is dynamic we must re-add every header for that
        // key — including static ones (e.g. duplicate History-Info entries) —
        // or the statics added at startup would be lost after the first dial.
        let dynamic_keys: HashSet<&str> = self
            .custom_headers
            .iter()
            .filter(|(_, tpl)| tpl.is_dynamic())
            .map(|(key, _)| key.as_str())
            .collect();

        for key in &dynamic_keys {
            self.phone.rm_header(key);
        }
        for (key, tpl) in &self.custom_headers {
            if dynamic_keys.contains(key.as_str()) {
                self.phone.add_header(key, &tpl.render(&ctx));
            }
        }
    }

    pub fn notify(&self, summary: &str, body: &str) {
        if !self.notify_enabled {
            return;
        }
        let body_with_profile = format!("[{}] {}", self.profile_name, body);
        crate::notify::send(summary, &body_with_profile);
    }

    /// Close every overlay (Logs / Help / Call history / Contacts). Callers open
    /// exactly one afterwards, keeping overlays mutually exclusive.
    pub(super) fn close_overlays(&mut self) {
        self.log.show = false;
        self.log.search_mode = false;
        self.log.search_query.clear();
        self.help_show = false;
        self.call_history.show = false;
        self.call_history.delete_confirm = None;
        self.contacts_state.show = false;
        self.contacts_state.delete_confirm = None;
        self.confirm_yes = false;
        self.log.scroll = 0;
    }

    pub(super) fn refresh_log(&mut self) {
        if let Some(path) = &self.log.path {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.log.lines = content.lines().map(sanitize_log_line).collect();
            }
        }
    }
}

/// Clean a raw log line for display: emulate a terminal's carriage-return
/// overwrite (keep only what follows the last `\r`, e.g. a curl progress meter's
/// final state) and drop ANSI escape sequences and other control characters that
/// would otherwise garble the rendering.
fn sanitize_log_line(raw: &str) -> String {
    let tail = raw.rsplit('\r').next().unwrap_or(raw);
    let mut out = String::with_capacity(tail.len());
    let mut chars = tail.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // CSI sequence: ESC '[' … final byte in '@'..='~'. Other escapes: drop
            // the escape and its next byte, best-effort.
            if chars.peek() == Some(&'[') {
                chars.next();
                for d in chars.by_ref() {
                    if ('@'..='~').contains(&d) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else if c == '\t' {
            out.push(' ');
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phone::Phone;
    use std::sync::{Arc, Mutex};

    #[test]
    fn sanitize_log_line_strips_cr_and_ansi() {
        // Carriage-return overwrite keeps only the final segment.
        assert_eq!(sanitize_log_line("old text\rnew text"), "new text");
        // ANSI colour codes are removed, printable text survives.
        assert_eq!(sanitize_log_line("\x1b[32mok\x1b[0m"), "ok");
        // Tabs become spaces; other control chars are dropped.
        assert_eq!(sanitize_log_line("a\tb\x07c"), "a bc");
        // A plain line is untouched.
        assert_eq!(sanitize_log_line("plain"), "plain");
    }

    #[derive(Clone, Default)]
    struct RecordingPhone {
        log: Arc<Mutex<Vec<String>>>,
    }
    impl Phone for RecordingPhone {
        fn register(&self, _: &str, _: u32) {}
        fn dial(&self, _: &str) {}
        fn hangup(&self) {}
        fn hangup_all(&self) {}
        fn accept(&self) {}
        fn hold(&self) {}
        fn resume(&self) {}
        fn mute(&self) {}
        fn send_dtmf(&self, _: char) {}
        fn switch_line(&self, _: usize) {}
        fn transfer(&self, _: &str) {}
        fn attended_transfer_start(&self, _: &str) {}
        fn attended_transfer_exec(&self) {}
        fn attended_transfer_abort(&self) {}
        fn add_header(&self, key: &str, value: &str) {
            self.log.lock().unwrap().push(format!("add {key}={value}"));
        }
        fn rm_header(&self, key: &str) {
            self.log.lock().unwrap().push(format!("rm {key}"));
        }
        fn set_audio_source(&self, _: &str) {}
        fn arm_invite_response(&self, _: u16, _: &str, _: Vec<String>) {}
        fn disarm_invite_response(&self) {}
    }

    fn app_with_headers(headers: Vec<(&str, &str)>, phone: RecordingPhone) -> App {
        App::new(
            "test".into(),
            "sip:test@example.com".into(),
            None,
            None,
            false,
            Box::new(phone),
            Theme::default(),
            Vec::new(),
            Profile::default(),
            Vec::new(),
            headers
                .into_iter()
                .map(|(k, v)| (k.to_string(), HeaderTemplate::new(v)))
                .collect(),
        )
    }

    #[test]
    fn refresh_re_adds_static_headers_sharing_a_dynamic_key() {
        let phone = RecordingPhone::default();
        let app = app_with_headers(
            vec![
                ("History-Info", "<sip:1@example.com>;index=1"),
                ("History-Info", "call-${uuid}"),
                ("X-Static", "keep-me"),
            ],
            phone.clone(),
        );

        app.refresh_dynamic_headers();

        let log = phone.log.lock().unwrap().clone();
        assert_eq!(
            log,
            vec![
                "rm History-Info".to_string(),
                "add History-Info=<sip:1@example.com>;index=1".to_string(),
                // dynamic value is a fresh UUID; assert only that it was re-added
                log[2].clone(),
            ]
        );
        assert!(log[2].starts_with("add History-Info=call-"));
        // A key with no dynamic template is left untouched (added once at startup).
        assert!(!log.iter().any(|l| l.contains("X-Static")));
    }

    #[test]
    fn refresh_ignores_keys_without_dynamic_templates() {
        let phone = RecordingPhone::default();
        let app = app_with_headers(vec![("X-Static", "keep-me")], phone.clone());

        app.refresh_dynamic_headers();

        assert!(phone.log.lock().unwrap().is_empty());
    }
}
