use crate::{config::Hook, config::Theme, phone::Phone, profile::Profile};
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

pub struct CallHistoryState {
    pub path: Option<PathBuf>,
    pub entries: Vec<CallHistoryEntry>,
    pub show: bool,
    pub selected: usize,
    pub search_query: String,
    pub search_mode: bool,
}

impl CallHistoryState {
    /// Indices into `entries` that match the current search query.
    pub fn filtered_indices(&self) -> Vec<usize> {
        let q = self.search_query.to_lowercase();
        if q.is_empty() {
            return (0..self.entries.len()).collect();
        }
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.peer.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }
}

pub struct LogState {
    pub entries: VecDeque<String>,
    pub scroll: usize,
    pub show: bool,
    pub baresip_path: Option<PathBuf>,
    pub show_baresip: bool,
    pub baresip_lines: Vec<String>,
    /// Last known visible height (set during render, used to clamp scroll).
    pub visible_height: usize,
}

pub struct CommandState {
    pub active: bool,
    pub input: String,
    pub error: Option<String>,
    /// Prefix typed before Tab was first pressed (for cycling through matches).
    pub tab_prefix: Option<String>,
    pub tab_index: usize,
}

// ─── App ──────────────────────────────────────────────────────────────────────

pub struct App {
    pub profile_name: String,
    pub account_aor: String,
    pub reg_status: RegStatus,
    pub calls: Vec<Call>,
    pub selected_call: usize,
    pub muted: bool,
    pub notify_enabled: bool,
    pub transfer_mode: TransferMode,
    pub tick: u64,
    pub dial: DialState,
    pub mwi: MwiState,
    pub call_history: CallHistoryState,
    pub log: LogState,
    pub last_call_reason: Option<String>,
    pub command: CommandState,
    pub(crate) phone: Box<dyn Phone>,
    pub quit: bool,
    pub quit_confirm: bool,
    pub switch_to: bool,
    pub edit_profile: bool,
    pub theme: Theme,
    pub hooks: Vec<Hook>,
    pub profile: Profile,
}

impl App {
    pub fn new(
        profile_name: String,
        account_aor: String,
        baresip_log_path: Option<PathBuf>,
        call_history_path: Option<PathBuf>,
        notify_enabled: bool,
        phone: Box<dyn Phone>,
        theme: Theme,
        hooks: Vec<Hook>,
        profile: Profile,
    ) -> Self {
        Self {
            profile_name,
            account_aor,
            reg_status: RegStatus::Unknown,
            calls: Vec::new(),
            selected_call: 0,
            muted: false,
            notify_enabled,
            transfer_mode: TransferMode::None,
            tick: 0,
            phone,
            quit: false,
            quit_confirm: false,
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
            },
            log: LogState {
                entries: VecDeque::with_capacity(200),
                scroll: 0,
                show: false,
                baresip_path: baresip_log_path,
                show_baresip: false,
                baresip_lines: Vec::new(),
                visible_height: 0,
            },
            last_call_reason: None,
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
        }
    }

    pub fn push_log(&mut self, msg: impl Into<String>) {
        if self.log.entries.len() >= 200 {
            self.log.entries.pop_front();
        }
        self.log.entries.push_back(msg.into());
        self.log.scroll = 0; // auto-scroll to bottom on new entry
    }

    pub fn notify(&self, summary: &str, body: &str) {
        if !self.notify_enabled {
            return;
        }
        let body_with_profile = format!("[{}] {}", self.profile_name, body);
        if let Err(e) = std::process::Command::new("notify-send")
            .args([
                "-a",
                "ringo",
                "-i",
                "call-start",
                summary,
                &body_with_profile,
            ])
            .spawn()
        {
            crate::rlog!(Debug, "notify-send failed: {}", e);
        }
    }

    pub(super) fn refresh_baresip_log(&mut self) {
        if let Some(path) = &self.log.baresip_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.log.baresip_lines = content.lines().map(|l| l.to_string()).collect();
            }
        }
    }
}
