//! The QuickJS [`ScriptHost`]: owns the `rquickjs` runtime + context and the
//! scenario registry, and knows how to run the top-level pass (which registers
//! scenarios) and a single scenario. The neutral [`crate::engine::run`] drives it.
//!
//! Persisting JS callbacks is the interesting part: `scenario(name, fn)` stashes
//! the JS closure as a [`Persistent<Function>`] in the shared [`Registry`]; later
//! `run_scenario` restores it inside a fresh `ctx.with(...)` and calls it with the
//! per-scenario context returned by `setup()` (or `undefined`). The whole host lives
//! on one `spawn_blocking` thread, so the single-threaded QuickJS context never moves.

use crate::engine::ctx::Ctx as EngineCtx;
use crate::engine::mock_server::BridgedRequest;
use crate::engine::{ScenarioInfo, ScenarioResult, ScriptHost, TopLevel};
use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::{
    AsyncContext, AsyncRuntime, Ctx as JsCtx, Error as JsError, Function, Module, Persistent, Value,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// The per-file env map (`--env-file` + sibling `<stem>.env`), mutable so
/// `loadEnv(...)` can extend it at run time (mirrors the rhai frontend).
pub type EnvVars = Arc<Mutex<HashMap<String, String>>>;

/// Scenarios registered during the top-level pass: their metadata plus the
/// persisted JS body. Shared (via `Arc`) between the `scenario(...)` global and the
/// host. The `Persistent` form is `'static`, so it survives across `ctx.with(...)`.
#[derive(Default)]
pub struct Registry {
    scenarios: Mutex<Vec<(ScenarioInfo, Persistent<Function<'static>>)>>,
    setup: Mutex<Option<Persistent<Function<'static>>>>,
    teardown: Mutex<Option<Persistent<Function<'static>>>>,
    /// Dynamic mock responders: the request receiver + the persisted JS closure.
    /// Pumped on the scenario thread (see `pump_bridged`), since QuickJS is `!Send`.
    pub(super) bridged: Mutex<
        Vec<(
            mpsc::Receiver<BridgedRequest>,
            Persistent<Function<'static>>,
        )>,
    >,
}

impl Registry {
    pub fn add(&self, info: ScenarioInfo, body: Persistent<Function<'static>>) {
        self.scenarios.lock().unwrap().push((info, body));
    }
    /// Register a dynamic mock responder (its request channel + JS closure).
    pub fn add_bridged(
        &self,
        rx: mpsc::Receiver<BridgedRequest>,
        body: Persistent<Function<'static>>,
    ) {
        self.bridged.lock().unwrap().push((rx, body));
    }
    pub fn set_setup(&self, body: Persistent<Function<'static>>) {
        *self.setup.lock().unwrap() = Some(body);
    }
    pub fn set_teardown(&self, body: Persistent<Function<'static>>) {
        *self.teardown.lock().unwrap() = Some(body);
    }
    fn setup_fn(&self) -> Option<Persistent<Function<'static>>> {
        self.setup.lock().unwrap().clone()
    }
    fn teardown_fn(&self) -> Option<Persistent<Function<'static>>> {
        self.teardown.lock().unwrap().clone()
    }
    fn infos(&self) -> Vec<ScenarioInfo> {
        self.scenarios
            .lock()
            .unwrap()
            .iter()
            .map(|(i, _)| i.clone())
            .collect()
    }
}

pub struct JsHost {
    rt: AsyncRuntime,
    context: AsyncContext,
    registry: Arc<Registry>,
    source: String,
    label: String,
    /// The entry file's absolute path, used as its ES-module id so relative
    /// `import`s resolve against its directory regardless of the process cwd.
    entry_name: String,
}

// The QuickJS `Runtime`/`Context` (and the `Persistent` bodies in the registry)
// are `!Send` — and that's fine: `engine::run` no longer requires `ScriptHost:
// Send`. A `JsHost` is built by the per-file `build` closure *on the
// `spawn_blocking` thread* and only ever used and dropped there, so it never
// crosses a thread boundary. No `unsafe impl Send` needed.

impl JsHost {
    pub fn new(
        engine: Arc<EngineCtx>,
        source: String,
        label: String,
        env: EnvVars,
        overrides: HashMap<String, String>,
        base_dir: PathBuf,
    ) -> anyhow::Result<Self> {
        // The async runtime/context turns the blocking waiters (until/verifyAudio)
        // into JS Promises so `Promise.all` runs them concurrently. We drive its event
        // loop with `futures_executor::block_on` (not tokio) on this thread, so the
        // engine's existing sync `block_on` verbs don't trip a nested-runtime panic.
        let rt = AsyncRuntime::new()?;
        // Wire ES-module `import`: resolve specifiers on the real filesystem (cwd-
        // independent), so a scenario can `import` shared helper files.
        futures_executor::block_on(rt.set_loader(FsResolver, FsLoader));
        let context = futures_executor::block_on(AsyncContext::full(&rt))?;
        // The entry's module id is its absolute path, so relative `import`s resolve
        // against its directory. The file name is cosmetic (the source is supplied
        // inline); only the parent directory matters for resolution.
        let entry_name = std::fs::canonicalize(&base_dir)
            .unwrap_or_else(|_| base_dir.clone())
            .join(
                Path::new(&label)
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("scenario.js")),
            )
            .to_string_lossy()
            .into_owned();
        // `Registry` holds `!Send` `Persistent` bodies; the `Arc` is only ever
        // shared between this host and the `scenario(...)` JS closure, both confined
        // to the single QuickJS thread.
        #[allow(clippy::arc_with_non_send_sync)]
        let registry = Arc::new(Registry::default());
        futures_executor::block_on(context.with(|ctx| {
            super::bindings::install(&ctx, &engine, &registry, &env, &overrides, &base_dir)
        }))?;
        Ok(Self {
            rt,
            context,
            registry,
            source,
            label,
            entry_name,
        })
    }

    /// Parse-check the entry module without evaluating it — mirrors the rhai
    /// frontend's compile-only `check`: `Module::declare` surfaces syntax errors but
    /// runs no top-level code, so `env(...)`/`scenario(...)`/`import` never fire and
    /// neither env vars nor baresip are needed. The declared module is discarded.
    pub fn check_syntax(&self) -> Result<(), String> {
        let name = self.entry_name.clone();
        let source = self.source.clone();
        let label = self.label.clone();
        futures_executor::block_on(
            self.context.async_with(async move |ctx| {
                match Module::declare(ctx.clone(), name, source) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(super::dsl::core::format_exception(&ctx, e, &label)),
                }
            }),
        )
    }
}

impl ScriptHost for JsHost {
    fn run_top_level(&mut self) -> TopLevel {
        let label = self.label.clone();
        let name = self.entry_name.clone();
        let source = self.source.clone();
        // Evaluate the file as an ES module so it can `import` helper files: top-level
        // `scenario(...)` calls register persisted bodies; a no-scenario file's
        // top-level code IS the scenario and runs here. Module eval returns a promise
        // (a module may use top-level `await`), which we drive to completion.
        let top: Result<(), String> =
            futures_executor::block_on(self.context.async_with(async move |ctx| {
                match Module::evaluate(ctx.clone(), name, source) {
                    Ok(promise) => promise
                        .into_future::<()>()
                        .await
                        .map_err(|e| super::dsl::core::format_exception(&ctx, e, &label)),
                    Err(e) => Err(super::dsl::core::format_exception(&ctx, e, &label)),
                }
            }));

        let scenarios = self.registry.infos();
        if scenarios.is_empty() {
            TopLevel::Single(top)
        } else {
            TopLevel::Suite {
                scenarios,
                top_error: top.err(),
            }
        }
    }

    fn run_scenario(&mut self, name: &str) -> ScenarioResult {
        let label = self.label.clone();
        let registry = self.registry.clone();
        // `async_with` so the body's Promise (an `async (ctx) => …` body that `await`s
        // the async verbs) can be driven to completion here. A plain sync body returns
        // `undefined`, which `MaybePromise` resolves immediately — both shapes work.
        futures_executor::block_on(self.context.async_with(async |ctx| {
            let entry = registry
                .scenarios
                .lock()
                .unwrap()
                .iter()
                .find(|(i, _)| i.name == name)
                .map(|(_, f)| f.clone());
            let Some(persistent) = entry else {
                return ScenarioResult::Failed(format!("scenario `{name}` not registered"));
            };
            let body: Function = match persistent.restore(&ctx) {
                Ok(f) => f,
                Err(e) => return ScenarioResult::Failed(format!("restore `{name}`: {e}")),
            };
            // The per-scenario context is whatever `setup()` returns (a fixture the
            // body and teardown share); `undefined` when there is no setup. Agents
            // themselves resolve by name in the engine, independent of this value.
            let mut sctx = Value::new_undefined(ctx.clone());
            // `setup()` (if any) runs first; its failure fails the scenario before
            // the body, and no teardown runs (nothing was set up).
            if let Some(setup) = registry.setup_fn() {
                if let Ok(f) = setup.restore(&ctx) {
                    match await_value(&f, ()).await {
                        Ok(v) => sctx = v,
                        Err(e) => {
                            return super::dsl::scenario::classify_scenario_error(&ctx, e, &label);
                        }
                    }
                }
            }
            let result = match await_body(&body, (sctx.clone(),)).await {
                Ok(()) => ScenarioResult::Passed,
                Err(e) => super::dsl::scenario::classify_scenario_error(&ctx, e, &label),
            };
            // `teardown(ctx)` runs regardless of the body's outcome (so fixtures can be
            // torn down); its own error shouldn't mask the body's result.
            if let Some(teardown) = registry.teardown_fn() {
                if let Ok(f) = teardown.restore(&ctx) {
                    let _ = await_body(&f, (sctx,)).await;
                }
            }
            result
        }))
    }
}

/// Call a scenario function and, if it returned a Promise (an `async` body), await
/// it; a sync body's `undefined` return resolves immediately. A synchronous throw
/// surfaces as the call `Err`; a rejected Promise surfaces from the await.
async fn await_body<'js, A>(f: &Function<'js>, args: A) -> rquickjs::Result<()>
where
    A: rquickjs::function::IntoArgs<'js>,
{
    f.call::<_, rquickjs::promise::MaybePromise<'js>>(args)?
        .into_future::<()>()
        .await
}

/// Like [`await_body`] but keeps the resolved value — used for `setup()`, whose
/// return becomes the per-scenario context passed to the body.
async fn await_value<'js, A>(f: &Function<'js>, args: A) -> rquickjs::Result<Value<'js>>
where
    A: rquickjs::function::IntoArgs<'js>,
{
    f.call::<_, rquickjs::promise::MaybePromise<'js>>(args)?
        .into_future::<Value<'js>>()
        .await
}

/// Resolves ES-module specifiers on the real filesystem using absolute paths, so
/// `import`s work regardless of the process cwd. Relative specifiers (`./`, `../`)
/// resolve against the importing module's directory; bare or absolute specifiers are
/// taken as given. A missing `.js` extension is implied.
struct FsResolver;

impl Resolver for FsResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &JsCtx<'js>,
        base: &str,
        name: &str,
        _attrs: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<String> {
        let target = if name.starts_with('.') {
            Path::new(base)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(name)
        } else {
            PathBuf::from(name)
        };
        let target = if target.extension().is_some() {
            target
        } else {
            target.with_extension("js")
        };
        std::fs::canonicalize(&target)
            .map(|p| p.to_string_lossy().into_owned())
            .map_err(|_| JsError::new_resolving(base.to_string(), name.to_string()))
    }
}

/// Reads a resolved module file from disk and declares it.
struct FsLoader;

impl Loader for FsLoader {
    fn load<'js>(
        &mut self,
        ctx: &JsCtx<'js>,
        name: &str,
        _attrs: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<Module<'js, Declared>> {
        let source = std::fs::read(name).map_err(|_| JsError::new_loading(name.to_string()))?;
        Module::declare(ctx.clone(), name, source)
    }
}

impl Drop for JsHost {
    fn drop(&mut self) {
        // Persistent values must be cleared before the runtime is freed, or QuickJS
        // asserts on a non-empty GC object list at teardown.
        self.registry.scenarios.lock().unwrap().clear();
        *self.registry.setup.lock().unwrap() = None;
        *self.registry.teardown.lock().unwrap() = None;
        self.registry.bridged.lock().unwrap().clear();
        let _ = &self.rt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ctx::Ctx as EngineCtx;
    use crate::engine::{ScriptHost, TopLevel};
    use crate::runtime::report::{Human, Level, Reporter};
    use std::time::Duration;

    fn make_host(source: &str) -> JsHost {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let reporter: Box<dyn Reporter + Send> = Box::new(Human::new(Level::Quiet));
        let ctx = Arc::new(EngineCtx::new(
            rt.handle().clone(),
            reporter,
            Duration::from_secs(5),
        ));
        // Keep the runtime alive for the host's lifetime.
        std::mem::forget(rt);
        JsHost::new(
            ctx,
            source.to_string(),
            "test.js".to_string(),
            Arc::new(Mutex::new(HashMap::new())),
            HashMap::new(),
            PathBuf::from("."),
        )
        .unwrap()
    }

    #[test]
    fn scenario_each_registers_one_scenario_per_row() {
        let src = r#"
          scenario.each([
            { name: "alpha", n: 1 },
            { name: "beta",  n: 2 },
            { name: "gamma", n: 3 },
          ])("param: $name", { tags: ["param"] }, (ctx, p) => {
            log("row " + p.name + " n=" + p.n);
          });
        "#;
        let mut host = make_host(src);
        let top = host.run_top_level();
        let scenarios = match top {
            TopLevel::Suite {
                scenarios,
                top_error,
            } => {
                assert!(top_error.is_none(), "top-level error: {:?}", top_error);
                scenarios
            }
            other => panic!("expected Suite, got {:?}", other),
        };
        assert_eq!(scenarios.len(), 3, "should register 3 scenarios");
        assert_eq!(scenarios[0].name, "param: alpha");
        assert_eq!(scenarios[1].name, "param: beta");
        assert_eq!(scenarios[2].name, "param: gamma");
        assert_eq!(scenarios[0].tags, vec!["param".to_string()]);
    }

    #[test]
    fn scenario_each_without_opts() {
        let src = r#"
          scenario.each([{ k: "x" }, { k: "y" }])("no-opts: $k", (ctx, p) => {
            log(p.k);
          });
        "#;
        let mut host = make_host(src);
        let top = host.run_top_level();
        let scenarios = match top {
            TopLevel::Suite { scenarios, .. } => scenarios,
            other => panic!("expected Suite, got {:?}", other),
        };
        assert_eq!(scenarios.len(), 2);
        assert_eq!(scenarios[0].name, "no-opts: x");
        assert_eq!(scenarios[1].name, "no-opts: y");
    }

    #[test]
    fn scenario_each_passes_row_to_body() {
        // The body receives the row as its second argument; verify by stashing
        // it in a global and checking after run_scenario.
        let src = r#"
          var received = null;
          scenario.each([{ val: 42 }])("row check", (ctx, p) => {
            received = p.val;
          });
        "#;
        let mut host = make_host(src);
        let top = host.run_top_level();
        assert!(matches!(top, TopLevel::Suite { .. }));
        let result = host.run_scenario("row check");
        assert!(
            matches!(result, ScenarioResult::Passed),
            "scenario should pass, got {:?}",
            result
        );
    }
}
