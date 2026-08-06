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
    let h = cx
        .userdata::<HostState>()
        .expect("host state stored at install");
    h.eng.emit(&Event::Log { message: &msg });
}

/// Read a variable: the per-file env map (`--env-file`/`<scenario>.env`/`loadEnv`)
/// first, then the process environment; errors if unset.
#[ringo_flow_macros::ts_global(name = "env")]
pub(in crate::script::js) fn env_global(cx: Ctx<'_>, key: String) -> rquickjs::Result<String> {
    let h = cx
        .userdata::<HostState>()
        .expect("host state stored at install");
    if let Some(v) = h.env.lock().unwrap().get(&key) {
        return Ok(v.clone());
    }
    std::env::var(&key).map_err(|_| {
        super::super::convert::throw(&cx, &format!("environment variable `{key}` is not set"))
    })
}

/// Merge a dotenv file into this file's env at run time, resolved relative to the
/// scenario's directory (later loads win).
#[ringo_flow_macros::ts_global(name = "loadEnv")]
pub(in crate::script::js) fn load_env_global(cx: Ctx<'_>, path: String) -> rquickjs::Result<()> {
    let h = cx
        .userdata::<HostState>()
        .expect("host state stored at install");
    let p = h.base.join(&path);
    crate::script::dotenv::merge_dotenv(&p, &mut h.env.lock().unwrap())
        .map_err(|e| super::super::convert::throw(&cx, &e.to_string()))
}

/// Set the default `until` timeout for the rest of the script (e.g. `"10s"`).
#[ringo_flow_macros::ts_global(name = "defaultTimeout")]
pub(in crate::script::js) fn default_timeout_global(
    cx: Ctx<'_>,
    duration: String,
) -> rquickjs::Result<()> {
    let d =
        duration::parse_duration(&duration).map_err(|e| super::super::convert::throw(&cx, &e))?;
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
pub(in crate::script::js) async fn wait_async<'js>(
    cx: Ctx<'js>,
    seconds: i64,
) -> rquickjs::Result<()> {
    let eng = cx
        .userdata::<HostState>()
        .expect("host state stored at install")
        .eng
        .clone();
    let secs = seconds.max(0) as u64;
    eng.emit(&Event::Wait {
        seconds: secs as f64,
    });
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
        Err(e) => Err(super::super::convert::throw(
            &cx,
            &format!("wait task failed: {e}"),
        )),
    }
}

// The async `until` poll loop. Mirrors the engine's `assertion::await_until`
// (25ms ticks, refcounted assert-silencing, same timeout message) but yields
// between ticks instead of sleeping the thread — so concurrent waiters under
// `Promise.all` overlap. Each tick first drains pending dynamic mock requests
// (the responder closure can only run on this, the scenario thread). Silencing
// is refcounted (begin/end), so overlapping `until`s under `Promise.all` no
// longer clobber each other's stashed assertions.
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
        Some(s) => {
            duration::parse_duration(&s).map_err(|e| super::super::convert::throw(&cx, &e))?
        }
        None => eng.default_timeout(),
    };
    eng.begin_assert_silent();
    let deadline = Instant::now() + timeout;
    let outcome: Result<Value, (String, bool)> = loop {
        super::mock::pump_bridged(&cx, &reg);
        match cond.call::<_, Value>(()) {
            Ok(v) => break Ok(v),
            // A thrown assertion is the "not yet" signal — retry until the deadline.
            // But a JS programming error (TypeError/ReferenceError/SyntaxError) will
            // never come true by retrying, so surface it at once as a scenario bug
            // instead of spinning to a timeout with an opaque message.
            Err(JsError::Exception) => {
                let (text, fatal) = caught_error(&cx);
                if fatal {
                    break Err((text, true));
                }
                if Instant::now() >= deadline {
                    break Err((text, false));
                }
            }
            Err(e) => break Err((e.to_string(), false)),
        }
        // Yield ~25ms without entering a tokio runtime on this thread: drive the
        // sleep on the engine's runtime and await its handle (which wakes our
        // futures-executor driver).
        let _ = eng.rt.spawn(async { tokio::time::sleep(TICK).await }).await;
    };
    eng.end_assert_silent();
    eng.emit_last_assert();
    outcome.map_err(|(e, fatal)| {
        let msg = if fatal {
            format!(
                "condition threw a JS error — a bug in the condition, not an unmet assertion:\n{e}"
            )
        } else {
            format!("not satisfied within {timeout:?}: {e}")
        };
        super::super::convert::throw(&cx, &msg)
    })
}

/// Text of the currently-pending JS exception (after a call returned
/// `Err(Exception)`), best-effort.
pub(in crate::script::js) fn exception_text(ctx: &Ctx<'_>) -> String {
    let caught = ctx.catch();
    if let Some(ex) = caught.as_exception() {
        let msg = ex
            .message()
            .or_else(|| ex.to_string().into())
            .unwrap_or_else(|| "exception".into());
        // Append the JS stack (`    at … (file:line)`) when QuickJS provides one, so
        // parse/eval and scenario-body failures point at the offending file:line.
        match ex.stack().filter(|s| !s.trim().is_empty()) {
            Some(s) => format!("{msg}\n{}", s.trim_end()),
            None => msg,
        }
    } else if let Some(s) = caught.as_string() {
        s.to_string().unwrap_or_else(|_| "exception".into())
    } else {
        "exception".into()
    }
}

/// Catch the pending JS exception and classify it: the message text plus whether it
/// is *fatal* — a JS programming error (`TypeError`/`ReferenceError`/`SyntaxError`)
/// that retrying can never satisfy, unlike an assertion's "not yet" throw (a plain
/// string thrown by the engine). Clears the pending exception.
fn caught_error(ctx: &Ctx<'_>) -> (String, bool) {
    let caught = ctx.catch();
    if let Some(obj) = caught.as_object() {
        let name = obj.get::<_, String>("name").ok();
        let message = obj
            .get::<_, String>("message")
            .ok()
            .filter(|m| !m.is_empty());
        let fatal = matches!(
            name.as_deref(),
            Some("TypeError" | "ReferenceError" | "SyntaxError")
        );
        let stack = obj
            .get::<_, String>("stack")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let mut text = match (name, message) {
            (Some(n), Some(m)) => format!("{n}: {m}"),
            (Some(n), None) => n,
            (None, Some(m)) => m,
            (None, None) => "exception".into(),
        };
        if let Some(s) = stack {
            // QuickJS `stack` is just the frames (`    at … (file:line)`), no message
            // line — append it so the failure points at the offending expression.
            text.push('\n');
            text.push_str(s.trim_end());
        }
        return (text, fatal);
    }
    let text = caught
        .as_string()
        .and_then(|s| s.to_string().ok())
        .unwrap_or_else(|| "exception".into());
    (text, false)
}

/// Format any QuickJS error from an `eval`/`call` into a human string, catching the
/// pending exception so its message (and the file label) are surfaced.
pub fn format_exception(ctx: &Ctx<'_>, err: JsError, label: &str) -> String {
    match err {
        JsError::Exception => format!("in {label}: {}", exception_text(ctx)),
        other => format!("in {label}: {other}"),
    }
}
