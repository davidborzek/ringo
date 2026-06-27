use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use anyhow::Result;

use crate::account::{Account, BackendOptions};
use crate::event::{AppEvent, InviteHeaders};
use crate::phone::Phone;

pub use crate::baresip::BaresipBackend;
pub use crate::baresip::call_count;
pub use crate::baresip::is_registered;
pub use crate::baresip::received_audio;
pub use crate::baresip::sent_audio;

/// Shut down the backend's global runtime: hang up calls, tear down the user
/// agents, stop the event loop and join its thread. Call once at process exit;
/// a no-op if the backend was never started. Agnostic façade over the concrete
/// backend's teardown (the FFI backend stops its libre event thread here).
pub fn shutdown() {
    crate::baresip::stop_re_thread();
}

/// A backend provides SIP user-agent functionality — library init, event
/// translation and the phone command interface. The FFI backend links
/// libbaresip directly (statically with `vendored`, or dynamically via
/// pkg-config).
pub trait Backend: Send {
    /// Spawn the backend (process or library), start I/O tasks on `rt`, and
    /// return a [`Session`] handle. The session owns the event stream, phone
    /// command interface, and optional header-polling closure. Connect retry
    /// happens internally; `AppEvent::BackendConnectFailed` lands in the event
    /// stream on failure.
    fn spawn_session(
        &self,
        rt: &tokio::runtime::Handle,
        name: &str,
        account: &Account,
        options: &BackendOptions,
    ) -> Result<Session>;
}

/// A live backend session. Dropping this tears down the backend (stops the
/// process / closes the library) and cleans up resources.
pub struct Session {
    /// Event stream (already translated to backend-neutral `AppEvent`s).
    pub events: Receiver<AppEvent>,
    /// Phone command interface.
    pub phone: Box<dyn Phone>,
    /// Log file path (for TUI display / debugging).
    pub log_path: Option<PathBuf>,
    /// Poll for inbound INVITE headers. Returns `None` when there is nothing
    /// new; `Some(headers)` when new headers have been parsed. `None` on the
    /// closure itself means the backend exposes headers directly in events
    /// (no trace polling needed).
    pub header_poll: Option<Box<dyn Fn() -> Option<InviteHeaders> + Send + Sync>>,
    /// Opaque handle — drop ends the backend session + cleanup.
    pub handle: Box<dyn Send>,
}

impl Session {
    pub fn new(
        events: Receiver<AppEvent>,
        phone: Box<dyn Phone>,
        log_path: Option<PathBuf>,
        header_poll: Option<Box<dyn Fn() -> Option<InviteHeaders> + Send + Sync>>,
        handle: Box<dyn Send>,
    ) -> Self {
        Self {
            events,
            phone,
            log_path,
            header_poll,
            handle,
        }
    }
}
