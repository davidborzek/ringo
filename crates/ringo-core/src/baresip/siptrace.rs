//! Optional SIP message tracing — every SIP request/response (sent + received)
//! is logged through the ringo log, for debugging signaling flows.
//!
//! baresip's own `uag_enable_sip_trace()` prints via `re_printf` to stdout
//! (with ANSI), which would corrupt ringo-flow's reporter/NDJSON output and
//! ringo-phone's TUI. So we install our OWN handler via libre's
//! `sip_set_trace_handler()` and route the trace through `rlog!` (→ whatever log
//! sink the binary configured: file, stderr, or none).

use std::os::raw::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};

use super::bindings::*;

/// Whether SIP tracing was requested (set by the binary before the RE thread
/// starts). The handler is installed once baresip is initialized.
static REQUESTED: AtomicBool = AtomicBool::new(false);

/// Request SIP tracing. Takes effect when the UA stack is initialized (the
/// handler is installed from `spawn_session`'s one-time init).
pub fn set_requested(on: bool) {
    REQUESTED.store(on, Ordering::Release);
}

/// libre SIP trace callback: log the raw message (it's text) with direction and
/// transport. Runs on the RE thread; a panic here would cross FFI, so guard it.
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
                unsafe { std::ffi::CStr::from_ptr(p) }.to_str().unwrap_or("?")
            }
        };
        crate::rlog!(Info, "SIP {dir} {transp}\n{}", msg.trim_end());
    }));
}

/// Install the trace handler if tracing was requested. Called from the one-time
/// baresip init (already on the RE thread, after `ua_init`).
pub(super) fn install_if_requested() {
    if !REQUESTED.load(Ordering::Acquire) {
        return;
    }
    unsafe {
        let sip = uag_sip();
        if !sip.is_null() {
            sip_set_trace_handler(sip, Some(trace_cb));
        }
    }
}
