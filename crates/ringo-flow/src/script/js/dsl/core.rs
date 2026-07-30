//! Core scenario-runtime globals — `log`/`env`/`loadEnv`/`defaultTimeout`/`uuid`,
//! the async `wait(...)`/`until(...)` waiters — plus the shared [`HostState`]
//! the async free-fn globals reach through the context userdata, and the exception
//! formatting helpers.

use super::super::host::EnvVars;
use crate::engine::ctx::Ctx as EngineCtx;
use crate::engine::duration;
use crate::runtime::report::Event;
use rquickjs::function::Opt;
use rquickjs::{Ctx, Error as JsError, Function, JsLifetime, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Shared host state that every global reaches through the context userdata: the
/// engine, the scenario registry, the per-file env map, and the scenario's base
/// directory. `install` stores it once. An async global *must* read it (a capturing
/// closure can't express that its returned future borrows the call's `'js`), and the
/// sync globals do the same — so none of them capture.
pub(in crate::script::js) struct HostState {
    pub(in crate::script::js) eng: Arc<EngineCtx>,
    pub(in crate::script::js) reg: Arc<super::super::host::Registry>,
    pub(in crate::script::js) env: EnvVars,
    pub(in crate::script::js) base: PathBuf,
}
// All fields are `'static`, so the type never actually borrows `'js`.
unsafe impl<'js> JsLifetime<'js> for HostState {
    type Changed<'to> = HostState;
}

/// Print a timestamped note to the scenario log (and the `--json` stream).
#[ringo_flow_macros::ts_global(name = "log")]
pub(in crate::script::js) fn log_global(cx: Ctx<'_>, msg: String) {
    let h = cx.userdata::<HostState>().expect("host state stored at install");
    h.eng.emit(&Event::Log { message: &msg });
}

/// Read a variable: the per-file env map (`--env-file`/`<scenario>.env`/`loadEnv`)
/// first, then the process environment; errors if unset.
#[ringo_flow_macros::ts_global(name = "env")]
pub(in crate::script::js) fn env_global(cx: Ctx<'_>, key: String) -> rquickjs::Result<String> {
    let h = cx.userdata::<HostState>().expect("host state stored at install");
    if let Some(v) = h.env.lock().unwrap().get(&key) {
        return Ok(v.clone());
    }
    std::env::var(&key)
        .map_err(|_| super::super::convert::throw(&cx, &format!("environment variable `{key}` is not set")))
}

/// Merge a dotenv file into this file's env at run time, resolved relative to the
/// scenario's directory (later loads win).
#[ringo_flow_macros::ts_global(name = "loadEnv")]
pub(in crate::script::js) fn load_env_global(cx: Ctx<'_>, path: String) -> rquickjs::Result<()> {
    let h = cx.userdata::<HostState>().expect("host state stored at install");
    let p = h.base.join(&path);
    crate::script::dotenv::merge_dotenv(&p, &mut h.env.lock().unwrap())
        .map_err(|e| super::super::convert::throw(&cx, &e.to_string()))
}

/// Set the default `until` timeout for the rest of the script (e.g. `"10s"`).
#[ringo_flow_macros::ts_global(name = "defaultTimeout")]
pub(in crate::script::js) fn default_timeout_global(cx: Ctx<'_>, duration: String) -> rquickjs::Result<()> {
    let d = duration::parse_duration(&duration).map_err(|e| super::super::convert::throw(&cx, &e))?;
    cx.userdata::<HostState>()
        .expect("host state stored at install")
        .eng
        .set_default_timeout(d);
    Ok(())
}

/// A fresh random UUID v4 string.
#[ringo_flow_macros::ts_global(name = "uuid")]
pub(in crate::script::js) fn uuid_global() -> String {
    uuid::Uuid::new_v4().to_string()
}

// Async free fn (reads `eng` from userdata). Snapshots each session's state watcher
// (without holding the sessions lock across the hold) and runs `wait_holding` on the
// runtime, awaiting it so the JS loop keeps turning.
/// Hold for N seconds; rejects if a call that is established at the start drops.
#[ringo_flow_macros::ts_global(name = "wait")]
pub(in crate::script::js) async fn wait_async<'js>(cx: Ctx<'js>, seconds: i64) -> rquickjs::Result<()> {
    let eng = cx
        .userdata::<HostState>()
        .expect("host state stored at install")
        .eng
        .clone();
    let secs = seconds.max(0) as u64;
    eng.emit(&Event::Wait { seconds: secs as f64 });
    let watchers = {
        let sessions = eng.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions
            .iter()
            .map(|(name, s)| (name.clone(), s.state()))
            .collect::<Vec<_>>()
    };
    match eng
        .rt
        .spawn(crate::runtime::wait_holding(
            Duration::from_secs(secs),
            watchers,
        ))
        .await
    {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(super::super::convert::throw(&cx, &e.to_string())),
        Err(e) => Err(super::super::convert::throw(&cx, &format!("wait task failed: {e}"))),
    }
}

// The async `until` poll loop. Mirrors the engine's `assertion::await_until`
// (25ms ticks, global assert-silencing, same timeout message) but yields between
// ticks instead of sleeping the thread — so concurrent waiters under `Promise.all`
// overlap. Each tick first drains pending dynamic mock requests (the responder
// closure can only run on this, the scenario thread). Note: assert-silencing is
// global, so overlapping two `until`s can cross their silencing — keep
// `Promise.all` to independent verbs (e.g. `verifyAudio`) where it matters.
/// Resolves with `cond`'s value once it stops throwing, or rejects on timeout.
/// `await` it (reads as `await until(...)`); the resolved value lets `.value()` bind a
/// verified value. While waiting it yields the event loop, so several `until`/
/// `verifyAudio` can run under `await Promise.all([...])`.
#[ringo_flow_macros::ts_global(name = "until")]
pub(in crate::script::js) async fn until_async<'js>(
    cx: Ctx<'js>,
    #[jsdoc(type = "() => unknown")] cond: Function<'js>,
    within: Opt<String>,
) -> rquickjs::Result<Value<'js>> {
    const TICK: Duration = Duration::from_millis(25);
    let (eng, reg) = {
        let st = cx
            .userdata::<HostState>()
            .expect("host state stored at install");
        (st.eng.clone(), st.reg.clone())
    };
    let timeout = match within.0 {
        Some(s) => duration::parse_duration(&s).map_err(|e| super::super::convert::throw(&cx, &e))?,
        None => eng.default_timeout(),
    };
    eng.set_assert_silent(true);
    let deadline = Instant::now() + timeout;
    let outcome = loop {
        super::mock::pump_bridged(&cx, &reg);
        match cond.call::<_, Value>(()) {
            Ok(v) => break Ok(v),
            // A thrown assertion is the "not yet" signal; `exception_text` clears the
            // pending exception so it can't leak into the next tick.
            Err(JsError::Exception) => {
                let e = exception_text(&cx);
                if Instant::now() >= deadline {
                    break Err(e);
                }
            }
            Err(e) => break Err(e.to_string()),
        }
        // Yield ~25ms without entering a tokio runtime on this thread: drive the
        // sleep on the engine's runtime and await its handle (which wakes our
        // futures-executor driver).
        let _ = eng.rt.spawn(async { tokio::time::sleep(TICK).await }).await;
    };
    eng.set_assert_silent(false);
    eng.emit_last_assert();
    outcome.map_err(|e| super::super::convert::throw(&cx, &format!("not satisfied within {timeout:?}: {e}")))
}

/// Text of the currently-pending JS exception (after a call returned
/// `Err(Exception)`), best-effort.
pub(in crate::script::js) fn exception_text(ctx: &Ctx<'_>) -> String {
    let caught = ctx.catch();
    if let Some(ex) = caught.as_exception() {
        ex.message()
            .or_else(|| ex.to_string().into())
            .unwrap_or_else(|| "exception".into())
    } else if let Some(s) = caught.as_string() {
        s.to_string().unwrap_or_else(|_| "exception".into())
    } else {
        "exception".into()
    }
}

/// Format any QuickJS error from an `eval`/`call` into a human string, catching the
/// pending exception so its message (and the file label) are surfaced.
pub fn format_exception(ctx: &Ctx<'_>, err: JsError, label: &str) -> String {
    match err {
        JsError::Exception => format!("in {label}: {}", exception_text(ctx)),
        other => format!("in {label}: {other}"),
    }
}
