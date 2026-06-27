//! Baresip backend — direct FFI bindings to libbaresip + libre.
//!
//! This module provides raw FFI declarations for the C API functions
//! ringo needs, plus a `BaresipBackend` that implements `Backend` by
//! calling libbaresip directly (no process spawning, no ctrl_tcp wire protocol).
//!
//! Architecture:
//! - `libre_init()` + `ua_init()` + `conf_configure_buf()` on init
//! - `re_main()` runs on a dedicated thread (the RE thread)
//! - Commands run on the RE thread via `re_thread_enter/leave`
//! - Events arrive via `bevent_register()` callback, translated to `AppEvent`
//! - All baresip modules (codecs, audio drivers, …) are statically linked
//!   into libbaresip.a and resolved via `lookup_static_module()` — no dlopen
//! - Inbound INVITE headers are extracted in the `BEVENT_SIPSESS_CONN` handler
//!   (all headers from `msg->hdrl`) and surfaced via the `header_poll` closure

mod ausrc;
mod bindings;
mod config;
mod events;
mod phone;
mod re_thread;
mod sounds;

use std::collections::HashMap;
use std::ffi::CString;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use anyhow::{Result, bail};

use crate::account::{Account, BackendOptions};
use crate::backend::{Backend, Session};
use crate::event::AppEvent;

use self::bindings::*;
use self::config::{build_config_string, configure_account};
use self::events::bevent_handler;
use self::phone::{BaresipPhone, BaresipSessionHandle, make_header_poll};
use self::re_thread::{EVENT_TX, enter_re_thread, on_re_thread, redirect_logging, start_re_thread};

pub use self::re_thread::stop_re_thread;

/// Returns true if any UA is still registered. Checks `uag_list()` on the
/// RE thread without holding the lock. Used by ringo-flow to wait for
/// `ua_unregister` to complete between scenarios.
pub fn is_registered() -> bool {
    use self::bindings::{Ua, ua_isregistered, uag_list};
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    on_re_thread(move || {
        let result = unsafe {
            let list = uag_list();
            if list.is_null() {
                false
            } else {
                let mut le = (*list).head;
                let mut found = false;
                while !le.is_null() {
                    let ua = (*le).data as *const Ua;
                    if !ua.is_null() && ua_isregistered(ua) {
                        found = true;
                        break;
                    }
                    le = (*le).next;
                }
                found
            }
        };
        let _ = tx.send(result);
    });
    rx.recv().unwrap_or(false)
}

/// Recently received (decoded) mono audio for the UA with audio key `key`
/// (the account username), plus its sample rate. Captured in-process by ringo's
/// own audio player, so ringo-flow can verify a received tone without reading
/// baresip's sndfile recordings. `None` if no audio has been received yet.
pub fn received_audio(key: &str) -> Option<(Vec<i16>, u32)> {
    ausrc::received_window(key)
}

/// Recently sent (rendered) mono audio for the UA with audio key `key`, plus its
/// sample rate. Only populated when full capture is enabled (`--save-audio`);
/// otherwise empty. Used by ringo-flow to save the sent recording.
pub fn sent_audio(key: &str) -> Option<(Vec<i16>, u32)> {
    ausrc::sent_window(key)
}

/// Returns the total number of active calls across all UAs. Used by
/// ringo-flow to wait for BYE flush before dropping sessions.
pub fn call_count() -> u32 {
    use self::bindings::uag_call_count;
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel();
    on_re_thread(move || {
        let count = unsafe { uag_call_count() };
        let _ = tx.send(count);
    });
    rx.recv().unwrap_or(0)
}

/// Backend that uses libbaresip directly via FFI (no process spawning,
/// no ctrl_tcp wire protocol). libre and libbaresip are built from the
/// vendored submodules and statically linked into the binary.
pub struct BaresipBackend;

impl Backend for BaresipBackend {
    fn spawn_session(
        &self,
        _rt: &tokio::runtime::Handle,
        name: &str,
        account: &Account,
        options: &BackendOptions,
    ) -> Result<Session> {
        // Ensure EVENT_TX exists
        let _ = EVENT_TX.set(Mutex::new(HashMap::new()));

        // Redirect libre/baresip debug output to stderr
        redirect_logging();

        // Start libre + re_main on a dedicated RE thread (once, idempotent)
        if let Err(e) = start_re_thread() {
            bail!("{e}");
        }

        // All baresip API calls must run on the RE thread.
        let mut ua_ptr: *mut Ua = std::ptr::null_mut();
        let msg_rx;
        unsafe {
            // RAII guard: re_thread_leave() runs on drop, including on panic or
            // any early `bail!` below — so a failure can't deadlock the RE thread.
            let _guard = enter_re_thread();

            // Set conf_path to a per-process temp dir so baresip does NOT read
            // ~/.baresip/accounts or ~/.baresip/config (our config is in-memory
            // via conf_configure_buf). baresip only writes its `uuid` file here;
            // 0700 keeps it private on a shared host. Removed on shutdown
            // (stop_re_thread), so a clean exit leaves nothing in /tmp.
            let dir = format!("/tmp/ringo-baresip-{}", std::process::id());
            {
                use std::os::unix::fs::DirBuilderExt;
                let _ = std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(&dir);
            }
            let tmp = CString::new(dir).unwrap();
            let _ = conf_path_set(tmp.as_ptr());

            let config_str = build_config_string(account, options);
            crate::rlog!(Info, "baresip config:\n{}", config_str);
            let config_c = match CString::new(config_str) {
                Ok(s) => s,
                Err(_) => bail!("generated baresip config contains an interior NUL byte"),
            };
            let rc = conf_configure_buf(config_c.as_ptr() as *const u8, config_c.to_bytes().len());
            if rc != 0 {
                bail!("conf_configure_buf() failed (rc={rc})");
            }

            // baresip_init + ua_init: only once (not per-session)
            static BARESIP_INIT_DONE: OnceLock<bool> = OnceLock::new();
            if !BARESIP_INIT_DONE.get().copied().unwrap_or(false) {
                let cfg = conf_config();
                if cfg.is_null() {
                    bail!("conf_config() returned null");
                }
                let rc = baresip_init(cfg);
                if rc != 0 {
                    bail!("baresip_init() failed (rc={rc})");
                }

                let sw = CString::new("ringo").unwrap();
                let rc = ua_init(sw.as_ptr(), true, true, true);
                if rc != 0 {
                    bail!("ua_init() failed (rc={rc})");
                }

                bevent_register(Some(bevent_handler), std::ptr::null_mut());

                // Register ringo's own audio source + player module (persistent
                // per-UA source that survives re-INVITEs — see ausrc.rs).
                if let Err(e) = ausrc::register_module() {
                    bail!("{e}");
                }

                let _ = BARESIP_INIT_DONE.set(true);
            }

            // Load statically compiled modules (from config "module" lines).
            // All modules — including the audio driver (pulse/coreaudio) — are
            // linked into libbaresip.a, so module_load() resolves them via
            // lookup_static_module() without ever hitting dlopen. The
            // audio_driver is already set by build_config_string from
            // RINGO_DEFAULT_AUDIO, so no runtime override is needed.
            let rc = conf_modules();
            if rc != 0 {
                crate::rlog!(Warn, "conf_modules() returned {rc}");
            }

            // Alloc UA with the account AOR.
            // In headless mode, route BOTH the source and player through ringo's
            // own audio module (see ausrc.rs), with a per-UA device key. The
            // SOURCE is persistent per-UA, survives re-INVITEs, and is race-free
            // across the parallel UAs in this single process. The PLAYER is
            // self-clocked so the RX decode/record pipeline always advances —
            // aubridge's player only clocks when paired with an aubridge source,
            // which no longer exists. Only when audio_driver is "aubridge"
            // (headless) — not None (ringo-phone uses real audio like pipewire).
            let audio_params = if options.audio_driver.as_deref() == Some("aubridge") {
                // Full per-call capture (sent + received) only when recordings
                // are wanted (--save-audio); otherwise just the rolling verify
                // window is retained.
                ausrc::set_full_capture(options.record_audio);
                ausrc::init_generator(&account.username);
                format!(
                    ";audio_source=ringo,{};audio_player=ringo,{}",
                    account.username, account.username
                )
            } else {
                String::new()
            };
            let aor = match CString::new(format!(
                "{}<sip:{}@{}>{}{}",
                account
                    .display_name
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("{s} "))
                    .unwrap_or_default(),
                account.username,
                account.domain,
                account
                    .transport
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!(";transport={s}"))
                    .unwrap_or_default(),
                audio_params,
            )) {
                Ok(s) => s,
                Err(_) => bail!("account fields contain an interior NUL byte"),
            };

            let rc = ua_alloc(&mut ua_ptr, aor.as_ptr());
            if rc != 0 {
                bail!("ua_alloc() failed (rc={rc})");
            }

            // Configure the account: auth, outbound, STUN, mediaenc, etc.
            let acc = ua_account(ua_ptr);
            if !acc.is_null() {
                if let Err(e) = configure_account(acc, account) {
                    bail!("configure_account() failed: {e}");
                }
            }

            // Register event sender BEFORE ua_register — ua_register fires
            // RegisterOk synchronously, so the handler needs the sender
            // in EVENT_TX to route it.
            let ua_usize = ua_ptr as usize;
            let (msg_tx, rx) = std::sync::mpsc::channel::<AppEvent>();
            msg_rx = rx;
            if let Some(mtx) = EVENT_TX.get() {
                mtx.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(ua_usize, msg_tx);
            }

            // Register
            let rc = ua_register(ua_ptr);
            if rc != 0 {
                crate::rlog!(Warn, "ua_register() failed (rc={rc})");
            }
            // _guard drops here → re_thread_leave()
        }

        let ua_usize = ua_ptr as usize;

        let log_path: Option<PathBuf> = Some(PathBuf::from(format!("/tmp/ringo-{}.log", name)));

        // The registry key for ringo's audio source module (only in aubridge
        // mode; with real audio the source isn't ours and set_audio_source falls
        // back to baresip's transient audio_set_source).
        let audio_key = if options.audio_driver.as_deref() == Some("aubridge") {
            Some(account.username.clone())
        } else {
            None
        };
        let phone = Box::new(BaresipPhone::new(ua_usize, audio_key.clone()));
        let handle = Box::new(BaresipSessionHandle::new(ua_usize, audio_key));
        let header_poll = Some(make_header_poll(ua_usize));

        Ok(Session::new(msg_rx, phone, log_path, header_poll, handle))
    }
}
