use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};

use crate::event::AppEvent;

use super::bindings::*;
use super::re_thread::EVENT_TX;

/// Inbound INVITE headers keyed by (UA pointer, SIP Call-ID); value is the
/// ordered list of (header-name, header-value) pairs from the INVITE.
type InboundHeaderMap = std::collections::HashMap<(usize, String), Vec<(String, String)>>;

/// Global store for inbound INVITE headers. Populated at BEVENT_SIPSESS_CONN
/// (before ua_accept creates the call), consumed by the header_poll closure
/// (filtered by UA pointer).
static INBOUND_HEADERS: OnceLock<Mutex<InboundHeaderMap>> = OnceLock::new();

pub fn inbound_headers_store() -> &'static Mutex<InboundHeaderMap> {
    INBOUND_HEADERS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// A custom SIP response armed for a UA's next inbound INVITE(s), instead of
/// accepting the call. The deflection (302 + Contact) helper is just one shape
/// of this generic response.
#[derive(Clone)]
struct ArmedResponse {
    scode: u16,
    reason: String,
    /// Extra header lines (without trailing CRLF), e.g. `Contact: <sip:…>`.
    headers: Vec<String>,
}

/// UA pointer → armed response. Sticky (like baresip-apps `redirect`): stays
/// until [`disarm_invite_response`] or the UA is gone.
static ARMED_RESPONSES: OnceLock<Mutex<std::collections::HashMap<usize, ArmedResponse>>> =
    OnceLock::new();

fn armed_responses() -> &'static Mutex<std::collections::HashMap<usize, ArmedResponse>> {
    ARMED_RESPONSES.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Arm `ua` so its next inbound INVITE is answered with `scode`/`reason` plus
/// `headers`, instead of being accepted. Sticky until [`disarm_invite_response`].
pub(crate) fn arm_invite_response(ua: usize, scode: u16, reason: String, headers: Vec<String>) {
    armed_responses()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(
            ua,
            ArmedResponse {
                scode,
                reason,
                headers,
            },
        );
}

/// Clear any armed response for `ua` (subsequent INVITEs are accepted normally).
pub(crate) fn disarm_invite_response(ua: usize) {
    armed_responses()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&ua);
}

/// DTMF digits received per UA, accumulated in arrival order from
/// `BEVENT_CALL_DTMF_START` (baresip wires the per-call handler itself).
static RECEIVED_DTMF: OnceLock<Mutex<std::collections::HashMap<usize, String>>> = OnceLock::new();

fn received_dtmf_store() -> &'static Mutex<std::collections::HashMap<usize, String>> {
    RECEIVED_DTMF.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// All DTMF digits received on `ua` so far, in order (e.g. `"1234#"`).
pub(crate) fn received_dtmf(ua: usize) -> String {
    received_dtmf_store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&ua)
        .cloned()
        .unwrap_or_default()
}

/// Drop a UA's received-DTMF buffer (on session teardown).
pub(crate) fn clear_dtmf(ua: usize) {
    received_dtmf_store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&ua);
}

/// Take (read + remove) the stored INVITE headers for `(ua, call_id)`.
/// Empty if none were captured (e.g. no such call).
pub(crate) fn inbound_headers(ua: usize, call_id: &str) -> Vec<(String, String)> {
    inbound_headers_store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&(ua, call_id.to_string()))
        .unwrap_or_default()
}

/// Answer an inbound INVITE `msg` directly with `scode`/`reason` + extra
/// `headers`, the way baresip rejects pre-call INVITEs (`sip_treplyf`,
/// fire-and-forget — `stp`/`mbp` NULL, no transaction to free). Must run on the
/// RE thread (called from the bevent handler). `false` on a NUL byte or send
/// error. The `"%s"` format keeps `%` inside header values from being read as
/// printf specifiers.
fn respond_to_invite(msg: *const SipMsg, scode: u16, reason: &str, headers: &[String]) -> bool {
    let mut body = String::new();
    for h in headers {
        body.push_str(h);
        body.push_str("\r\n");
    }
    body.push_str("Content-Length: 0\r\n\r\n");
    let (Ok(reason_c), Ok(body_c)) = (CString::new(reason), CString::new(body)) else {
        return false;
    };
    let rc = unsafe {
        sip_treplyf(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            uag_sip(),
            msg,
            false,
            scode,
            reason_c.as_ptr(),
            c"%s".as_ptr(),
            body_c.as_ptr(),
        )
    };
    rc == 0
}

/// Callback for list_apply: collect each SIP header name+value.
unsafe extern "C" fn collect_hdr_cb(le: *mut Le, _arg: *mut c_void) -> bool {
    if le.is_null() {
        return false;
    }
    let hdr = unsafe { (*le).data as *const SipHdr };
    if hdr.is_null() {
        return false;
    }
    let collector = unsafe { &mut *((_arg) as *mut HeaderVec) };
    let name = pl_to_string(unsafe { &(*hdr).name as *const Pl });
    let val = pl_to_string(unsafe { &(*hdr).val as *const Pl });
    collector.0.push((name, val));
    false // continue iterating
}

struct HeaderVec(Vec<(String, String)>);

/// baresip event callback — translates bevent types to `AppEvent`.
///
/// # Safety
/// This is called from C on the RE thread. A panic here would cross the FFI
/// boundary, which is UB — so the entire body is wrapped in `catch_unwind`.
pub unsafe extern "C" fn bevent_handler(ev: BeventEv, event: *mut Bevent, _arg: *mut c_void) {
    let result = panic::catch_unwind(AssertUnwindSafe(|| bevent_handler_inner(ev, event)));
    if let Err(_panic) = result {
        crate::rlog!(Error, "panic in bevent_handler — suppressed");
    }
}

fn bevent_handler_inner(ev: BeventEv, event: *mut Bevent) {
    let bevent_ev = ev as i32;

    let app_event = match bevent_ev {
        x if x == bevent_ev::BEVENT_SIPSESS_CONN as i32 => {
            // Incoming INVITE — extract ALL SIP headers from the message
            // and call ua_accept ourselves (call_accept=no in config).
            let msg = unsafe { bevent_get_msg(event) };
            // bevent_get_ua returns NULL for SIPSESS_CONN (no call yet).
            // Find the UA from the SIP message.
            let ua = if !msg.is_null() {
                unsafe { uag_find_msg(msg) }
            } else {
                std::ptr::null_mut()
            };
            if !msg.is_null() && !ua.is_null() {
                // Extract + store all INVITE headers first, keyed by UA — so the
                // header poll surfaces them even when we deflect (the call is
                // answered with a 302 and no call object is ever created, but a
                // scenario can still read the INVITE's custom headers).
                let call_id = pl_to_string(unsafe { &(*msg).callid as *const Pl });
                let mut collector = HeaderVec(Vec::new());
                unsafe {
                    list_apply(
                        &(*msg).hdrl as *const List,
                        true,
                        Some(collect_hdr_cb),
                        &mut collector as *mut HeaderVec as *mut c_void,
                    );
                }
                inbound_headers_store()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert((ua as usize, call_id), collector.0);

                // If this UA is armed with a custom response (e.g. a 302 deflect),
                // answer the INVITE directly and skip acceptance — no call object.
                let armed = armed_responses()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&(ua as usize))
                    .cloned();
                if let Some(r) = armed {
                    if respond_to_invite(msg, r.scode, &r.reason, &r.headers) {
                        crate::rlog!(
                            Info,
                            "answered inbound INVITE with {} {}",
                            r.scode,
                            r.reason
                        );
                    } else {
                        crate::rlog!(Warn, "failed to send armed {} response", r.scode);
                    }
                    // For 302 deflections, emit a CallDeflected event so the
                    // UI can show that a call was forwarded (no call object is
                    // ever created, so no CallIncoming/CallClosed pair fires).
                    if r.scode == 302 {
                        let from = pl_to_string(unsafe { &(*msg).from.auri as *const Pl });
                        let dname = pl_to_string(unsafe { &(*msg).from.dname as *const Pl });
                        let display_name = if dname.is_empty() { None } else { Some(dname) };
                        let target = r
                            .headers
                            .iter()
                            .find_map(|h| {
                                h.strip_prefix("Contact: <")
                                    .and_then(|s| s.split('>').next())
                                    .map(|s| s.to_string())
                            })
                            .unwrap_or_default();
                        let evt = AppEvent::CallDeflected {
                            from,
                            display_name,
                            target,
                        };
                        if let Some(mtx) = EVENT_TX.get() {
                            if let Some(tx) = mtx
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .get(&(ua as usize))
                            {
                                let _ = tx.send(evt);
                            }
                        }
                    }
                    // Stop propagation so nothing else accepts the call.
                    unsafe { bevent_stop(event) };
                    return;
                }

                // Accept the call (creates call object, sends 180 Ringing,
                // emits BEVENT_CALL_INCOMING).
                let rc = unsafe { ua_accept(ua, msg) };
                if rc != 0 {
                    crate::rlog!(Warn, "ua_accept() failed (rc={rc})");
                }
            }
            // SIPSESS_CONN is an internal event — don't forward to AppEvent.
            return;
        }
        x if x == bevent_ev::BEVENT_REGISTERING as i32 => {
            let ua = unsafe { bevent_get_ua(event) };
            let account = ua_aor(ua);
            AppEvent::Registering { account }
        }
        x if x == bevent_ev::BEVENT_REGISTER_OK as i32
            || x == bevent_ev::BEVENT_FALLBACK_OK as i32 =>
        {
            let ua = unsafe { bevent_get_ua(event) };
            let account = ua_aor(ua);
            AppEvent::RegisterOk { account }
        }
        x if x == bevent_ev::BEVENT_REGISTER_FAIL as i32
            || x == bevent_ev::BEVENT_FALLBACK_FAIL as i32 =>
        {
            let text = bevent_text(event);
            AppEvent::RegisterFailed { reason: text }
        }
        x if x == bevent_ev::BEVENT_UNREGISTERING as i32 => {
            let ua = unsafe { bevent_get_ua(event) };
            let account = ua_aor(ua);
            AppEvent::Unregistered { account }
        }
        x if x == bevent_ev::BEVENT_CALL_INCOMING as i32 => {
            let call = unsafe { bevent_get_call(event) };
            super::sounds::play_alert("ring.wav");
            let (call_id, number, display_name) = call_info(call);
            AppEvent::CallIncoming {
                call_id,
                number,
                display_name,
            }
        }
        x if x == bevent_ev::BEVENT_CALL_OUTGOING as i32 => {
            let call = unsafe { bevent_get_call(event) };
            let (call_id, number, _) = call_info(call);
            AppEvent::CallOutgoing { call_id, number }
        }
        x if x == bevent_ev::BEVENT_CALL_RINGING as i32 => {
            let call = unsafe { bevent_get_call(event) };
            super::sounds::play_alert("ringback.wav");
            let call_id = call_id_str(call);
            AppEvent::CallRinging { call_id }
        }
        x if x == bevent_ev::BEVENT_CALL_ESTABLISHED as i32 => {
            super::sounds::stop_alert();
            let call = unsafe { bevent_get_call(event) };
            let call_id = call_id_str(call);
            AppEvent::CallEstablished { call_id }
        }
        x if x == bevent_ev::BEVENT_CALL_CLOSED as i32 => {
            super::sounds::stop_alert();
            let call = unsafe { bevent_get_call(event) };
            // Snapshot RTP stats while the call still exists, so a scenario can
            // assert on call quality (MOS, loss, …) after hanging up.
            let ua = unsafe { bevent_get_ua(event) };
            if !ua.is_null() && !call.is_null() {
                super::stats::snapshot_on_close(ua as usize, call);
            }
            let call_id = call_id_str(call);
            let text = bevent_text(event);
            let scode = if !call.is_null() {
                unsafe { call_scode(call) }
            } else {
                0
            };
            let reason = if scode >= 100 && !text.starts_with(|c: char| c.is_ascii_digit()) {
                format!("{scode} {text}")
            } else {
                text
            };
            let error = crate::event::is_error_reason(&reason);
            AppEvent::CallClosed {
                call_id,
                reason,
                error,
            }
        }
        x if x == bevent_ev::BEVENT_CALL_DTMF_START as i32 => {
            // Inbound DTMF digit (baresip wires the per-call handler; the digit
            // is the event text). Accumulate per UA so a scenario can assert on
            // what was received. DTMF_END carries no digit — ignore it.
            let ua = unsafe { bevent_get_ua(event) };
            let digit = bevent_text(event);
            if !ua.is_null() && !digit.is_empty() {
                received_dtmf_store()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .entry(ua as usize)
                    .or_default()
                    .push_str(&digit);
            }
            return;
        }
        x if x == bevent_ev::BEVENT_MWI_NOTIFY as i32 => {
            let text = bevent_text(event);
            parse_mwi(&text)
        }
        _ => AppEvent::Unknown {
            class: "bevent".into(),
            type_: format!("{bevent_ev}"),
        },
    };

    // Route to the correct session by UA pointer.
    let ua = unsafe { bevent_get_ua(event) };
    if !ua.is_null() {
        if let Some(mtx) = EVENT_TX.get() {
            if let Some(tx) = mtx
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&(ua as usize))
            {
                let _ = tx.send(app_event);
            }
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub fn ua_aor(ua: *mut Ua) -> String {
    if ua.is_null() {
        return String::new();
    }
    let acc = unsafe { ua_account(ua) };
    if acc.is_null() {
        return String::new();
    }
    cstr_to_string(unsafe { account_aor(acc) })
}

pub fn call_info(call: *mut Call) -> (String, String, Option<String>) {
    if call.is_null() {
        return (String::new(), String::new(), None);
    }
    let id = call_id_str(call);
    let number = cstr_to_string(unsafe { call_peeruri(call) });
    let display_name = {
        let n = cstr_to_string(unsafe { call_peername(call) });
        if n.is_empty() { None } else { Some(n) }
    };
    (id, number, display_name)
}

pub fn call_id_str(call: *mut Call) -> String {
    if call.is_null() {
        return String::new();
    }
    cstr_to_string(unsafe { call_id(call) })
}

fn bevent_text(event: *mut Bevent) -> String {
    cstr_to_string(unsafe { bevent_get_text(event) })
}

pub fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .unwrap_or("")
        .to_string()
}

fn parse_mwi(param: &str) -> AppEvent {
    let mut waiting = false;
    let mut new_count = 0u32;
    for line in param.lines() {
        if let Some(val) = line.strip_prefix("Messages-Waiting:") {
            waiting = val.trim().eq_ignore_ascii_case("yes");
        }
        if let Some(val) = line.strip_prefix("Voice-Message:") {
            if let Some(new) = val.trim().split('/').next() {
                new_count = new.trim().parse().unwrap_or(0);
            }
        }
    }
    AppEvent::VoicemailStatus { waiting, new_count }
}

/// Convert a libre `pl` (pointer + length string slice) into an owned `String`.
fn pl_to_string(pl: *const Pl) -> String {
    if pl.is_null() {
        return String::new();
    }
    unsafe {
        let pl = &*pl;
        if pl.l == 0 {
            return String::new();
        }
        let slice = std::slice::from_raw_parts(pl.p as *const u8, pl.l);
        String::from_utf8_lossy(slice).to_string()
    }
}
