use std::collections::HashMap;
use std::os::raw::{c_char, c_int, c_void};
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::event::AppEvent;

use super::bindings::*;

/// Global event senders keyed by UA pointer (as usize). Each session
/// registers its sender here; `bevent_handler` routes events to the
/// correct session by looking up `bevent_get_ua(event) as usize`.
pub static EVENT_TX: OnceLock<Mutex<HashMap<usize, std::sync::mpsc::Sender<AppEvent>>>> =
    OnceLock::new();

/// Join handle for the dedicated RE thread. Module-level so `start_re_thread`
/// (which populates it) and `stop_re_thread` (which takes it to join the
/// thread) share the SAME instance — function-local statics would be two
/// distinct cells, leaving `stop_re_thread` a no-op.
static RE_HANDLE: OnceLock<Mutex<Option<std::thread::JoinHandle<()>>>> = OnceLock::new();

/// Whether the RE thread is up. A plain flag, NOT derived from locking
/// RE_HANDLE: `stop_re_thread` holds the RE_HANDLE lock while calling
/// `on_re_thread`, so reading it through RE_HANDLE would self-deadlock.
static RE_RUNNING: AtomicBool = AtomicBool::new(false);

/// Upper bound on the wait for the registrar to ack the final de-REGISTER on
/// shutdown. Reached only when the PBX is unresponsive; the happy path returns
/// as soon as the SIP stack drains (typically one round-trip).
const GRACEFUL_SHUTDOWN: std::time::Duration = std::time::Duration::from_millis(1500);

/// RAII guard that calls `re_thread_leave()` on drop — even if the closure
/// panics. Without this, a panic between `enter` and `leave` would permanently
/// block the RE thread (deadlock on next command).
pub struct ReThreadGuard;

impl Drop for ReThreadGuard {
    fn drop(&mut self) {
        unsafe { re_thread_leave() };
    }
}

/// Enter the RE thread and return a guard that calls `re_thread_leave()` on
/// drop (including on panic or early `?`/`bail!` return). Use this instead of
/// raw `re_thread_enter()` + manual `re_thread_leave()` when the block needs to
/// return values or use `?`, so a panic can't leave the RE thread deadlocked.
#[must_use]
pub fn enter_re_thread() -> ReThreadGuard {
    unsafe { re_thread_enter() };
    ReThreadGuard
}

/// Execute a closure on the RE thread (synchronous via re_thread_enter/leave).
///
/// # Panic safety
/// If `f` panics, `re_thread_leave()` is still called via the `ReThreadGuard`.
/// The panic propagates to the caller — but the RE thread is not left in a
/// deadlocked state.
pub fn on_re_thread<F: FnOnce()>(f: F) {
    // No-op if the RE thread was never started — e.g. a skipped scenario that
    // created no agents, whose teardown still polls is_registered()/call_count().
    // Without this, re_thread_enter/leave warn "re not ready" on stderr (the
    // log-redirect handler is only installed once the RE thread starts).
    if !re_thread_running() {
        return;
    }
    unsafe { re_thread_enter() };
    let _guard = ReThreadGuard;
    f();
}

/// Whether the dedicated RE thread has been started and not yet stopped.
fn re_thread_running() -> bool {
    RE_RUNNING.load(Ordering::Acquire)
}

/// Redirect libre/baresip debug output away from stdout/stderr.
///
/// 1. baresip's own log system (log.c) prints to stdout — disable that.
/// 2. libre's dbg_printf goes to stderr by default — install a handler
///    that routes warnings/errors through ringo's own logging system
///    (rlog! → /tmp/ringo-<profile>.log) instead of raw stderr.
pub fn redirect_logging() {
    static DBG_REDIRECTED: OnceLock<bool> = OnceLock::new();
    DBG_REDIRECTED.get_or_init(|| {
        unsafe extern "C" fn dbg_handler(
            level: c_int,
            p: *const c_char,
            len: usize,
            _arg: *mut c_void,
        ) {
            let _ = panic::catch_unwind(AssertUnwindSafe(|| {
                if p.is_null() || len == 0 {
                    return;
                }
                let slice = unsafe { std::slice::from_raw_parts(p as *const u8, len) };
                let msg = String::from_utf8_lossy(slice);
                // DBG_ERR=3, DBG_WARNING=4, DBG_INFO=2 — route to ringo log
                match level {
                    2 => crate::rlog!(Info, "libre: {}", msg.trim()),
                    3 => crate::rlog!(Error, "libre: {}", msg.trim()),
                    4 => crate::rlog!(Warn, "libre: {}", msg.trim()),
                    _ => {}
                }
            }));
        }

        // baresip log.c handler — routes module logs (STUN, ICE, aubridge,
        // RTP) through rlog! instead of stdout.
        static mut LOG_HANDLER: log = unsafe {
            log {
                le: std::mem::zeroed(),
                h: Some(baresip_log_handler),
            }
        };
        unsafe extern "C" fn baresip_log_handler(level: u32, msg: *const c_char) {
            let _ = panic::catch_unwind(AssertUnwindSafe(|| {
                if msg.is_null() {
                    return;
                }
                let s = unsafe { std::ffi::CStr::from_ptr(msg) };
                let msg = s.to_string_lossy();
                // LEVEL_DEBUG=0, LEVEL_INFO=1, LEVEL_WARN=2, LEVEL_ERROR=3
                match level {
                    0 => crate::rlog!(Debug, "baresip: {}", msg.trim()),
                    1 => crate::rlog!(Info, "baresip: {}", msg.trim()),
                    2 => crate::rlog!(Warn, "baresip: {}", msg.trim()),
                    3 => crate::rlog!(Error, "baresip: {}", msg.trim()),
                    _ => {}
                }
            }));
        }

        unsafe {
            // baresip log.c: disable stdout, enable info for module logs.
            log_enable_stdout(false);
            let enable_info = matches!(option_env!("RINGO_DEBUG_BARESIP"), Some("1"));
            log_enable_info(enable_info);
            log_enable_debug(false);

            // Register a log handler so baresip module logs go to rlog!
            // instead of being silently dropped (stdout disabled).
            log_register_handler(&raw mut LOG_HANDLER);

            // libre dbg.c: route through rlog! (set to DBG_INFO=2 for debug,
            // DBG_WARNING=4 for warnings only)
            let dbg_level = match option_env!("RINGO_DEBUG_BARESIP") {
                Some("1") => 2, // DBG_INFO — show STUN/ICE/registration logs
                _ => 4,         // DBG_WARNING — warnings and errors only
            };
            dbg_init(dbg_level, 0);
            dbg_handler_set(Some(dbg_handler), std::ptr::null_mut());
        }
        true
    });
}

/// Start libre + `re_main()` on a dedicated RE thread (once, idempotent).
///
/// `libre_init()` MUST be called on the same thread as `re_main()` — it
/// sets `re_global->tid` to the current thread.
pub fn start_re_thread() -> Result<(), String> {
    let handle_mutex = RE_HANDLE.get_or_init(|| Mutex::new(None));
    let mut guard = handle_mutex.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return Ok(()); // already started
    }

    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Option<String>>();
    let handle = std::thread::Builder::new()
        .name("baresip-re".into())
        .spawn(move || unsafe {
            if libre_init() != 0 {
                let _ = ready_tx.send(Some("libre_init() failed".into()));
                return;
            }
            if re_thread_async_init(4) != 0 {
                let _ = ready_tx.send(Some("re_thread_async_init() failed".into()));
                return;
            }
            let _ = ready_tx.send(None);
            re_main(None);
            // re_main returned (re_cancel from stop_re_thread). Tear down on
            // THIS thread — libre_init ran here, so libre_close must too, and it
            // must happen AFTER re_main, never before (freeing re_global while
            // re_main still polls it hangs the join). ua_stop_all already ran.
            ua_close();
            module_app_unload();
            conf_close();
            baresip_close();
            mod_close();
            re_thread_async_close();
            libre_close();
        })
        .map_err(|e| format!("failed to spawn RE thread: {e}"))?;

    // The RE thread sends exactly one readiness message after init. A recv
    // error means it died before signalling — surface that instead of panicking.
    match ready_rx.recv() {
        Ok(Some(e)) => return Err(e),
        Ok(None) => {}
        Err(_) => return Err("RE thread exited before init completed".into()),
    }
    *guard = Some(handle);
    RE_RUNNING.store(true, Ordering::Release);
    Ok(())
}

/// Stop the RE thread, deregistering cleanly first. The caller already fired the
/// un-REGISTER (dropping the session handle → ua_unregister), so:
///
/// 1. Wait — bounded — for the registrar to ack it: `is_registered()` stays true
///    until the un-REGISTER 200 OK arrives, and returns false fast when nothing
///    is registered (e.g. stopping from the picker). Runs on whatever thread
///    calls shutdown — behind the TUI's alternate screen on a genuine quit — so
///    we never leave a stale binding, and never wait longer than the round-trip.
/// 2. ua_stop_all(false) — graceful: hang up calls + destroy the (now
///    deregistered) UAs, but do NOT force-close the SIP stack. forced=true would
///    call sip_close(force), aborting an in-flight transaction; mid-registration
///    that double-derefs the registration → SIGTRAP. baresip's main.c uses the
///    graceful form and only forces as a last resort.
/// 3. re_cancel() + async poke — break out of re_main (blocked in epoll_wait, so
///    re_cancel alone won't wake it; post a dummy async event, the trick libre's
///    re_thread_leave uses).
/// 4. join() — the RE thread runs the rest of teardown (ua_close … libre_close)
///    after re_main; it MUST run there (where libre_init ran), so it can't live
///    here before the join.
pub fn stop_re_thread() {
    let handle_mutex = match RE_HANDLE.get() {
        Some(m) => m,
        None => return, // never started
    };
    let mut guard = handle_mutex.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        return;
    }

    // Wait for the in-flight de-REGISTER to be acked before stopping the loop, so
    // the registrar drops our binding instead of leaving it stale. Bounded so an
    // unresponsive PBX can't hang exit; returns immediately when nothing is
    // registered (e.g. quitting from the picker after a profile switch, whose
    // de-REGISTER already drained on the still-running RE thread).
    let deadline = std::time::Instant::now() + GRACEFUL_SHUTDOWN;
    while super::is_registered() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    on_re_thread(|| unsafe {
        ua_stop_all(false);
    });

    // Cancel re_main and join. The RE thread tears the rest down after re_main.
    unsafe {
        re_cancel();
        // re_cancel only sets polling=false; the RE thread is blocked in
        // epoll_wait and won't observe it. Post a dummy async event to write the
        // wakeup fd so fd_poll returns and re_main exits (same trick libre's
        // re_thread_leave uses). Without this the join below hangs forever.
        let _ = re_thread_async(None, None, std::ptr::null_mut());
    }
    if let Some(handle) = guard.take() {
        let _ = handle.join();
    }
    RE_RUNNING.store(false, Ordering::Release);

    // Clean up the temp dir created by THIS process only. The dir is already
    // PID-scoped, so this is safe even with several ringo instances running.
    // Profile log files (/tmp/ringo-<profile>.log) are intentionally left in
    // place — they are not PID-scoped, so deleting by glob would clobber the
    // logs of a concurrent instance, and they are useful for post-mortem
    // debugging. Log cleanup is the responsibility of whoever owns the name.
    let pid = std::process::id();
    let baresip_dir = format!("/tmp/ringo-baresip-{pid}");
    let _ = std::fs::remove_dir_all(&baresip_dir);
}
