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
// A parametrised scenario body receives the setup context plus the current
// table row: `(ctx: any, param: T) => void | Promise<void>`.
declare!("type ScenarioEachBody<T> = (ctx: any, param: T) => void | Promise<void>;");
// Factory returned by `scenario.each(table)`; calling it registers one scenario
// per table row, interpolating `$key` tokens in `name` from the row's fields.
declare!(
    "interface ScenarioEachFactory<T> {\n  (name: string, body: ScenarioEachBody<T>): void;\n  (name: string, opts: ScenarioOptions, body: ScenarioEachBody<T>): void;\n}"
);
// `scenario.each(table)(name, opts?, body)` — parametrised scenario registration.
// `name` interpolates `$key` tokens from each row; `body` receives `(setupCtx, row)`.
declare!(
    "declare namespace scenario {\n  function each<T>(table: T[]): ScenarioEachFactory<T>;\n}"
);

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

/// JS source for `scenario.each(table)(name, opts?, body)`. Evaluated after the
/// `scenario` global is installed, adding an `.each` property to it. Iterates the
/// table, interpolating `$key` tokens in `name` from each row's fields, and
/// registers one `scenario(...)` per row whose body receives `(setupCtx, row)`.
pub(in crate::script::js) const EACH_SHIM: &str = r#"
  scenario.each = function (table) {
    if (!Array.isArray(table)) throw new Error("scenario.each: table must be an array");
    return function (name, opts, body) {
      if (arguments.length === 2) { body = opts; opts = undefined; }
      if (typeof name !== "string") throw new Error("scenario.each: name must be a string");
      if (typeof body !== "function") throw new Error("scenario.each: body must be a function");
      for (var i = 0; i < table.length; i++) {
        var row = table[i];
        var label = name.replace(/\$(\w+)/g, function (_, k) {
          var v = row[k];
          return v == null ? "" : String(v);
        });
        scenario(label, opts, function (ctx) { return body(ctx, row); });
      }
    };
  };
"#;
