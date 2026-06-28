use std::ffi::CString;
use std::os::raw::{c_char, c_void};

use crate::event::InviteHeaders;
use crate::phone::Phone;

use super::bindings::*;
use super::re_thread::on_re_thread;

/// Phone implementation that calls libbaresip C functions directly.
/// All commands run on the RE thread via `re_thread_enter/leave`.
pub struct BaresipPhone {
    /// UA pointer stored as usize for Send-safety.
    ua: usize,
    /// Registry key for ringo's audio source module (the `<key>` in
    /// `audio_source=ringo,<key>`). `Some` in headless/aubridge mode; `None`
    /// with real audio, where `set_audio_source` falls back to baresip's
    /// transient `audio_set_source`.
    audio_key: Option<String>,
}

impl BaresipPhone {
    pub fn new(ua: usize, audio_key: Option<String>) -> Self {
        Self { ua, audio_key }
    }
}

impl Phone for BaresipPhone {
    fn register(&self, _aor: &str, _regint: u32) {
        let ua = self.ua;
        on_re_thread(move || {
            unsafe { ua_register(ua as *mut Ua) };
        });
    }

    fn dial(&self, number: &str) {
        let ua = self.ua;
        let num = match CString::new(number) {
            Ok(s) => s,
            Err(_) => return,
        };
        let num_len = number.len();
        on_re_thread(move || unsafe {
            // `num` is moved into the closure so its buffer stays alive for the
            // whole FFI call (the closure runs synchronously on the RE thread).
            let num_ptr = num.as_ptr();
            let ua_ptr = ua as *mut Ua;
            let acc = ua_account(ua_ptr);
            if acc.is_null() {
                return;
            }
            // Use baresip's account_uri_complete_strdup to complete the URI
            // exactly like baresip's menu module does.
            // "10000" → "sip:10000@example.com" (using the account's luri domain).
            let uri_pl = Pl {
                p: num_ptr,
                l: num_len,
            };
            let mut completed: *mut c_char = std::ptr::null_mut();
            let rc = account_uri_complete_strdup(acc, &mut completed, &uri_pl);
            if rc == 0 && !completed.is_null() {
                ua_connect(
                    ua_ptr,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    completed,
                    vidmode::VIDMODE_OFF,
                );
                mem_deref(completed as *mut c_void);
            }
        });
    }

    fn hangup(&self) {
        let ua = self.ua;
        on_re_thread(move || unsafe {
            let call = ua_call(ua as *mut Ua);
            if !call.is_null() {
                ua_hangup(ua as *mut Ua, call, 0, std::ptr::null());
            }
        });
    }

    fn hangup_all(&self) {
        let ua = self.ua;
        on_re_thread(move || unsafe {
            let calls = ua_calls(ua as *mut Ua);
            if !calls.is_null() {
                let mut le = (*calls).head;
                while !le.is_null() {
                    let call = (*le).data as *mut Call;
                    if !call.is_null() {
                        ua_hangup(ua as *mut Ua, call, 0, std::ptr::null());
                    }
                    le = (*le).next;
                }
            }
        });
    }

    fn accept(&self) {
        let ua = self.ua;
        on_re_thread(move || unsafe {
            let call = ua_call(ua as *mut Ua);
            if !call.is_null() {
                ua_answer(ua as *mut Ua, call, vidmode::VIDMODE_OFF);
            }
        });
    }

    fn hold(&self) {
        let ua = self.ua;
        on_re_thread(move || unsafe {
            let call = ua_call(ua as *mut Ua);
            if !call.is_null() {
                call_hold(call, true);
            }
        });
    }

    fn resume(&self) {
        let ua = self.ua;
        on_re_thread(move || unsafe {
            let call = ua_call(ua as *mut Ua);
            if !call.is_null() {
                call_hold(call, false);
            }
        });
    }

    fn mute(&self) {
        let ua = self.ua;
        on_re_thread(move || unsafe {
            let call = ua_call(ua as *mut Ua);
            if !call.is_null() {
                let audio = call_audio(call);
                if !audio.is_null() {
                    let muted = audio_ismuted(audio);
                    audio_mute(audio, !muted);
                }
            }
        });
    }

    fn send_dtmf(&self, digit: char) {
        let ua = self.ua;
        let key = digit as c_char;
        // KEYCODE_REL (0x04) signals key release — baresip's telev module
        // keeps the current digit "pressed" until a release or the next digit
        // arrives, so without sending KEYCODE_REL the last digit is never
        // completed (e.g. "*6" would leave the "6" hanging).
        const KEYCODE_REL: c_char = 0x04;
        on_re_thread(move || unsafe {
            let call = ua_call(ua as *mut Ua);
            if !call.is_null() {
                call_send_digit(call, key);
                call_send_digit(call, KEYCODE_REL);
            }
        });
    }

    fn switch_line(&self, line: usize) {
        let ua = self.ua;
        let linenum = line as u32;
        on_re_thread(move || unsafe {
            let calls = ua_calls(ua as *mut Ua);
            if !calls.is_null() {
                let call = call_find_linenum(calls, linenum);
                if !call.is_null() {
                    call_set_current(calls as *mut List, call);
                }
            }
        });
    }

    fn transfer(&self, uri: &str) {
        let ua = self.ua;
        let uri_c = match CString::new(uri) {
            Ok(s) => s,
            Err(_) => return,
        };
        let uri_ptr = uri_c.into_raw() as usize;
        on_re_thread(move || unsafe {
            let call = ua_call(ua as *mut Ua);
            if !call.is_null() {
                call_transfer(call, uri_ptr as *const c_char);
            }
            let _ = CString::from_raw(uri_ptr as *mut c_char);
        });
    }

    fn attended_transfer_start(&self, uri: &str) {
        // Place a consultation call to `uri`. The existing call is put on hold
        // automatically by baresip when a new call is started.
        self.dial(uri);
    }

    fn attended_transfer_exec(&self) {
        // Complete the attended transfer: REFER with Replaces.
        // The consultation call (current) replaces the held call.
        let ua = self.ua;
        on_re_thread(move || unsafe {
            let calls = ua_calls(ua as *mut Ua);
            if calls.is_null() {
                return;
            }
            let consult_call = ua_call(ua as *mut Ua);
            if consult_call.is_null() {
                return;
            }
            let mut le = (*calls).head;
            while !le.is_null() {
                let call = (*le).data as *mut Call;
                if !call.is_null() && call != consult_call {
                    call_replace_transfer(consult_call, call);
                    return;
                }
                le = (*le).next;
            }
        });
    }

    fn attended_transfer_abort(&self) {
        // Abort: hangup the consultation call (the held call can be resumed).
        let ua = self.ua;
        on_re_thread(move || unsafe {
            let call = ua_call(ua as *mut Ua);
            if !call.is_null() {
                ua_hangup(ua as *mut Ua, call, 0, std::ptr::null());
            }
        });
    }

    fn add_header(&self, key: &str, value: &str) {
        let ua = self.ua;
        // Reject CR/LF: baresip emits the header verbatim as "name: value\r\n"
        // with no sanitization, so a newline would inject arbitrary SIP headers.
        if key.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            crate::rlog!(Warn, "add_header: rejected CR/LF in header name or value");
            return;
        }
        let name_c = match CString::new(key) {
            Ok(s) => s,
            Err(_) => return,
        };
        let value_c = match CString::new(value) {
            Ok(s) => s,
            Err(_) => return,
        };
        let name_ptr = name_c.into_raw() as usize;
        let value_ptr = value_c.into_raw() as usize;
        let name_len = key.len();
        let value_len = value.len();
        on_re_thread(move || unsafe {
            let name = Pl {
                p: name_ptr as *const c_char,
                l: name_len,
            };
            let value = Pl {
                p: value_ptr as *const c_char,
                l: value_len,
            };
            ua_add_custom_hdr(ua as *mut Ua, &name, &value);
            let _ = CString::from_raw(name_ptr as *mut c_char);
            let _ = CString::from_raw(value_ptr as *mut c_char);
        });
    }

    fn rm_header(&self, key: &str) {
        let ua = self.ua;
        let name_c = match CString::new(key) {
            Ok(s) => s,
            Err(_) => return,
        };
        let name_ptr = name_c.into_raw() as usize;
        let name_len = key.len();
        on_re_thread(move || unsafe {
            let mut name = Pl {
                p: name_ptr as *const c_char,
                l: name_len,
            };
            ua_rm_custom_hdr(ua as *mut Ua, &mut name);
            let _ = CString::from_raw(name_ptr as *mut c_char);
        });
    }

    fn set_audio_source(&self, spec: &str) {
        // Headless/aubridge mode: route through ringo's own source module. The
        // registry is the single source of truth — persistent per-UA, applied
        // by the render thread, and re-read verbatim when baresip rebuilds the
        // stream on a re-INVITE (transfer / hold / line switch). No per-event
        // re-apply, no race.
        if let Some(key) = &self.audio_key {
            super::ausrc::set_generator(key, spec);
            return;
        }

        // Real-audio mode (ringo-phone): the source isn't ours, so fall back to
        // baresip's transient audio_set_source on the current call.
        let ua = self.ua;
        let parts: Vec<&str> = spec.splitn(2, ',').collect();
        if parts.len() != 2 {
            return;
        }
        let driver_c = match CString::new(parts[0]) {
            Ok(s) => s,
            Err(_) => return,
        };
        let device_c = match CString::new(parts[1]) {
            Ok(s) => s,
            Err(_) => return,
        };
        let driver_ptr = driver_c.into_raw() as usize;
        let device_ptr = device_c.into_raw() as usize;
        on_re_thread(move || unsafe {
            let call = ua_call(ua as *mut Ua);
            if !call.is_null() {
                let audio = call_audio(call);
                if !audio.is_null() {
                    audio_set_source(
                        audio,
                        driver_ptr as *const c_char,
                        device_ptr as *const c_char,
                    );
                }
            }
            let _ = CString::from_raw(driver_ptr as *mut c_char);
            let _ = CString::from_raw(device_ptr as *mut c_char);
        });
    }

    fn arm_invite_response(&self, scode: u16, reason: &str, headers: Vec<String>) {
        // Plain mutex update (no FFI) — run synchronously, not via on_re_thread,
        // so it's in place before a subsequent dial triggers the inbound INVITE.
        // The RE-thread bevent handler reads the same map under the lock.
        super::events::arm_invite_response(self.ua, scode, reason.to_string(), headers);
    }

    fn disarm_invite_response(&self) {
        super::events::disarm_invite_response(self.ua);
    }
    fn media_stats(&self) -> Option<crate::event::MediaStats> {
        super::stats::media_stats(self.ua)
    }
    fn received_dtmf(&self) -> String {
        super::events::received_dtmf(self.ua)
    }
}

/// Opaque handle — drop ends the backend session + cleanup.
pub struct BaresipSessionHandle {
    /// UA pointer stored as usize for Send-safety.
    ua: usize,
    /// Registry key for ringo's audio source module, removed on drop.
    audio_key: Option<String>,
}

impl BaresipSessionHandle {
    pub fn new(ua: usize, audio_key: Option<String>) -> Self {
        Self { ua, audio_key }
    }
}

impl Drop for BaresipSessionHandle {
    fn drop(&mut self) {
        let ua = self.ua;
        if let Some(mtx) = super::re_thread::EVENT_TX.get() {
            mtx.lock().unwrap_or_else(|e| e.into_inner()).remove(&ua);
        }
        if let Some(key) = &self.audio_key {
            super::ausrc::remove_generator(key);
        }
        super::stats::forget(ua);
        super::events::clear_dtmf(ua);
        // ua_unregister sends REGISTER expires=0 via sipreg_unregister and
        // waits for the 200 OK. ua_stop_register would just mem_deref the
        // sipreg without sending anything — leaving stale contacts on the PBX.
        // Do NOT call ua_destroy here — the UA is destroyed later by
        // ua_stop_all(true) in stop_re_thread (full shutdown sequence).
        on_re_thread(move || unsafe {
            ua_unregister(ua as *mut Ua);
        });
    }
}

/// Build the header-poll closure for the session. Returns inbound INVITE
/// headers for this UA only (extracted at BEVENT_SIPSESS_CONN time,
/// stored in the global INBOUND_HEADERS map keyed by UA pointer).
pub fn make_header_poll(ua: usize) -> Box<dyn Fn() -> Option<InviteHeaders> + Send + Sync> {
    Box::new(move || {
        let store = super::events::inbound_headers_store();
        let mut map = store.lock().unwrap_or_else(|e| e.into_inner());
        let result: InviteHeaders = map
            .iter()
            .filter(|((ua_id, _), _)| *ua_id == ua)
            .map(|((_, call_id), hdrs)| (call_id.clone(), hdrs.clone()))
            .collect();
        if result.is_empty() {
            None
        } else {
            // Drop the entries we just handed out so the store can't grow
            // unbounded over a long-running session.
            map.retain(|(ua_id, _), _| *ua_id != ua);
            Some(result)
        }
    })
}
