//! Optional SIP message tracing — every SIP request/response (sent + received)
//! written to its OWN sink (file or stderr), independent of the regular log.
//!
//! baresip's own `uag_enable_sip_trace()` prints via `re_printf` to stdout
//! (with ANSI), which would corrupt ringo-flow's reporter/NDJSON output and
//! ringo-phone's TUI. So we install our OWN handler via libre's
//! `sip_set_trace_handler()` and write to a dedicated sink the binary picks.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::raw::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use super::bindings::*;

/// Dedicated SIP-trace sink. Set ⇒ tracing is on (the handler is installed in
/// baresip init); unset ⇒ off. Separate from the regular log sink.
static SINK: OnceLock<Mutex<Box<dyn Write + Send>>> = OnceLock::new();

/// Trace SIP messages to `path` (created, truncated; parent dirs made). Call
/// before the first session is spawned. First call wins.
pub fn init_file(path: impl AsRef<Path>) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(f) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
    {
        let _ = SINK.set(Mutex::new(Box::new(f)));
    }
}

/// Trace SIP messages to stderr. Call before the first session is spawned.
pub fn init_stderr() {
    let _ = SINK.set(Mutex::new(Box::new(std::io::stderr())));
}

/// libre SIP trace callback: write the raw message (it's text) with direction
/// and transport to the trace sink. Runs on the RE thread; a panic here would
/// cross FFI, so guard it.
unsafe extern "C" fn trace_cb(
    tx: bool,
    tp: sip_transp,
    _src: *const sa,
    _dst: *const sa,
    pkt: *const u8,
    len: usize,
    _arg: *mut c_void,
) {
    let _ = panic::catch_unwind(AssertUnwindSafe(|| {
        let Some(mtx) = SINK.get() else {
            return;
        };
        if pkt.is_null() || len == 0 {
            return;
        }
        let bytes = unsafe { std::slice::from_raw_parts(pkt, len) };
        let msg = String::from_utf8_lossy(bytes);
        let dir = if tx { "TX →" } else { "RX ←" };
        let transp = {
            let p = unsafe { sip_transp_name(tp) };
            if p.is_null() {
                "?"
            } else {
                unsafe { std::ffi::CStr::from_ptr(p) }
                    .to_str()
                    .unwrap_or("?")
            }
        };
        if let Ok(mut w) = mtx.lock() {
            let ts = chrono::Local::now().format("%H:%M:%S%.3f");
            let _ = writeln!(w, "[{ts}] SIP {dir} {transp}\n{}\n", msg.trim_end());
        }
    }));
}

/// Install the trace handler if a sink was configured. Called from the one-time
/// baresip init (already on the RE thread, after `ua_init`).
pub(super) fn install_if_requested() {
    if SINK.get().is_none() {
        return;
    }
    unsafe {
        let sip = uag_sip();
        if !sip.is_null() {
            sip_set_trace_handler(sip, Some(trace_cb));
        }
    }
}
