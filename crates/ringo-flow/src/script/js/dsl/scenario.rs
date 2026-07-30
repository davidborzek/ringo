//! The scenario lifecycle DSL: the `scenario`/`skip`/`setup`/`teardown` registration
//! helpers and the error classification that turns a `skip(...)` marker into a
//! `Skipped` outcome. A scenario body receives the context returned by `setup()` (an
//! arbitrary fixture object), or `undefined` when there is no setup.

use super::super::tsgen::declare;
use super::core::HostState;
use crate::engine::{ScenarioInfo, ScenarioResult};
use rquickjs::function::Opt;
use rquickjs::{Ctx as JsCtx, Error as JsError, Function, Object, Persistent, Value};

// ── Scenario-domain TS types ──
/// The `scenario(name, { … }, body)` options. A single source of truth: the
/// `#[derive(TsInterface)]` exports the `ScenarioOptions` interface and a `FIELDS`
/// list. `skip` is a `boolean | string` union; it (and the rest) is parsed off the raw
/// object in [`scenario_info`], so this struct is a shape-only marker.
#[allow(dead_code)] // a codegen source: only its name + interface are used.
#[derive(ringo_flow_macros::TsInterface)]
struct ScenarioOptions {
    /// Tags for filtering with `--tag` / `--exclude-tag`.
    tags: Option<Vec<String>>,
    /// `true` to skip, or a string reason (reported, not run).
    #[jsdoc(type = "boolean | string", optional)]
    skip: Option<bool>,
    /// If any scenario sets `only: true`, only those run.
    only: Option<bool>,
}

// `ScenarioBody`'s `ctx` is whatever `setup()` returned (an arbitrary fixture), so it is
// typed `any`. No rquickjs binding to derive from → declared verbatim.
declare!("type ScenarioBody = (ctx: any) => void | Promise<void>;");

/// Abort the current scenario as *skipped* (reported, not failed).
// Thrown as a marker-prefixed string the host classifies.
#[ringo_flow_macros::ts_global(name = "skip")]
pub(in crate::script::js) fn skip_global(
    cx: JsCtx<'_>,
    reason: Opt<String>,
) -> rquickjs::Result<()> {
    let msg = format!("{SKIP_PREFIX}{}", reason.0.unwrap_or_default());
    Err(super::super::convert::throw(&cx, &msg))
}

/// Register a `setup(fn)` body, run before every scenario. Its return value becomes
/// the per-scenario context passed to the body (and to `teardown`).
#[ringo_flow_macros::ts_global(name = "setup")]
pub(in crate::script::js) fn setup_global<'js>(#[jsdoc(type = "() => any")] body: Function<'js>) {
    let reg = body
        .ctx()
        .userdata::<HostState>()
        .expect("host state stored at install")
        .reg
        .clone();
    reg.set_setup(Persistent::save(&body.ctx().clone(), body));
}

/// Register a `teardown(fn)` body, run after every scenario with the context that
/// `setup` returned.
#[ringo_flow_macros::ts_global(name = "teardown")]
pub(in crate::script::js) fn teardown_global<'js>(
    #[jsdoc(type = "(ctx: any) => void")] body: Function<'js>,
) {
    let reg = body
        .ctx()
        .userdata::<HostState>()
        .expect("host state stored at install")
        .reg
        .clone();
    reg.set_teardown(Persistent::save(&body.ctx().clone(), body));
}

/// Register a `scenario(...)`: with a third arg, `a` is the options object and
/// `b` the body; otherwise `a` is the body. Persists the body and records
/// tags/skip/only from the options.
#[ringo_flow_macros::ts_global(name = "scenario")]
#[jsdoc(sig = "scenario(name: string, body: ScenarioBody): void")]
#[jsdoc(sig = "scenario(name: string, opts: ScenarioOptions, body: ScenarioBody): void")]
pub(in crate::script::js) fn register_scenario<'a, 'b>(
    name: String,
    a: Value<'a>,
    b: Opt<Function<'b>>,
) -> rquickjs::Result<()> {
    let reg = a
        .ctx()
        .userdata::<HostState>()
        .expect("host state stored at install")
        .reg
        .clone();
    // Process each arm's body locally: mixing the two `Function`s into one binding
    // would force `'a` == `'b` (Function is invariant over its lifetime).
    match b.0 {
        Some(body) => {
            let info = scenario_info(name, a.as_object());
            let saved = Persistent::save(&body.ctx().clone(), body);
            reg.add(info, saved);
        }
        None => {
            let cx = a.ctx().clone();
            let body = a.into_function().ok_or_else(|| {
                super::super::convert::throw(&cx, "scenario body must be a function")
            })?;
            let saved = Persistent::save(&body.ctx().clone(), body);
            reg.add(
                ScenarioInfo {
                    name,
                    ..Default::default()
                },
                saved,
            );
        }
    }
    Ok(())
}

/// Parse a `scenario` options object (`{ tags, skip, only }`) into [`ScenarioInfo`].
fn scenario_info(name: String, opts: Option<&Object<'_>>) -> ScenarioInfo {
    let mut info = ScenarioInfo {
        name,
        ..Default::default()
    };
    let Some(opts) = opts else { return info };
    if let Ok(Some(tags)) = opts.get::<_, Option<Vec<String>>>("tags") {
        info.tags = tags;
    }
    // `skip` may be `true` or a `"reason"` string.
    if let Ok(v) = opts.get::<_, Value>("skip") {
        if let Some(b) = v.as_bool() {
            info.skip = b;
        } else if let Some(s) = v.as_string() {
            info.skip = true;
            info.skip_reason = s.to_string().ok();
        }
    }
    if let Ok(Some(only)) = opts.get::<_, Option<bool>>("only") {
        info.only = only;
    }
    info
}

/// Marker prefix a `skip(...)` throw carries, so the host tells it apart from a
/// real failure. Unlikely to appear in genuine error text.
const SKIP_PREFIX: &str = "\u{1f}RINGO_SKIP\u{1f}";

/// Classify a scenario body/setup error: a `skip(...)` marker becomes `Skipped`,
/// anything else `Failed` (with the file label).
pub fn classify_scenario_error(ctx: &JsCtx<'_>, err: JsError, label: &str) -> ScenarioResult {
    match err {
        JsError::Exception => {
            let msg = super::core::exception_text(ctx);
            match msg.strip_prefix(SKIP_PREFIX) {
                Some(reason) => {
                    ScenarioResult::Skipped((!reason.is_empty()).then(|| reason.into()))
                }
                None => ScenarioResult::Failed(format!("in {label}: {msg}")),
            }
        }
        other => ScenarioResult::Failed(format!("in {label}: {other}")),
    }
}
