//! The Rhai [`ScriptHost`]: holds the compiled program and the scenario registry,
//! and knows how to run the top-level pass and a single scenario. The neutral
//! [`crate::engine::run`] drives it.

use crate::engine::{ScriptHost, TopLevel};
use rhai::{AST, Dynamic, Engine, FnPtr, Scope};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// Scenario/`setup`/`teardown` closures registered during the top-level pass.
/// Shared (via `Arc`) between the registering Rhai functions and the host.
#[derive(Default)]
pub(super) struct Registry {
    scenarios: Mutex<Vec<(String, FnPtr)>>,
    setup: Mutex<Option<FnPtr>>,
    teardown: Mutex<Option<FnPtr>>,
    // Engine + AST for running closures on worker threads (the `parallel` verb),
    // filled once after compilation. The engine is held weakly to avoid a
    // reference cycle (the engine owns the verb closures that hold this registry).
    engine: OnceLock<Weak<Engine>>,
    ast: OnceLock<Arc<AST>>,
}

impl Registry {
    pub(super) fn add_scenario(&self, name: String, body: FnPtr) {
        self.scenarios.lock().unwrap().push((name, body));
    }
    pub(super) fn set_setup(&self, body: FnPtr) {
        *self.setup.lock().unwrap() = Some(body);
    }
    pub(super) fn set_teardown(&self, body: FnPtr) {
        *self.teardown.lock().unwrap() = Some(body);
    }
    fn take_scenarios(&self) -> Vec<(String, FnPtr)> {
        std::mem::take(&mut self.scenarios.lock().unwrap())
    }
    fn setup_fn(&self) -> Option<FnPtr> {
        self.setup.lock().unwrap().clone()
    }
    fn teardown_fn(&self) -> Option<FnPtr> {
        self.teardown.lock().unwrap().clone()
    }
    /// Record the engine + AST so the `parallel` verb can call closures on threads.
    pub(super) fn set_exec(&self, engine: Weak<Engine>, ast: Arc<AST>) {
        let _ = self.engine.set(engine);
        let _ = self.ast.set(ast);
    }
    /// The engine + AST, if still alive (the engine is parked, running this verb).
    pub(super) fn exec(&self) -> Option<(Arc<Engine>, Arc<AST>)> {
        Some((self.engine.get()?.upgrade()?, self.ast.get()?.clone()))
    }
}

pub(super) struct RhaiHost {
    engine: Arc<Engine>,
    ast: Arc<AST>,
    registry: Arc<Registry>,
    overrides: HashMap<String, String>,
    // Cached from the top-level pass, used by `run_scenario`.
    scenarios: Vec<(String, FnPtr)>,
    setup: Option<FnPtr>,
    teardown: Option<FnPtr>,
}

impl RhaiHost {
    pub(super) fn new(
        engine: Arc<Engine>,
        ast: Arc<AST>,
        registry: Arc<Registry>,
        overrides: HashMap<String, String>,
    ) -> Self {
        Self {
            engine,
            ast,
            registry,
            overrides,
            scenarios: Vec::new(),
            setup: None,
            teardown: None,
        }
    }
}

impl ScriptHost for RhaiHost {
    fn run_top_level(&mut self) -> TopLevel {
        let mut scope = Scope::new();
        // Call states are reached via the global `State` namespace; only the
        // user's `--set` overrides go into the bare scope here.
        for (key, value) in self.overrides.drain() {
            scope.push_constant(key, value);
        }
        let top = self.engine.run_ast_with_scope(&mut scope, &self.ast);
        self.scenarios = self.registry.take_scenarios();
        self.setup = self.registry.setup_fn();
        self.teardown = self.registry.teardown_fn();

        if self.scenarios.is_empty() {
            // No `scenario(…)` calls → the top-level code was the scenario.
            TopLevel::Single(top.map(|_| ()).map_err(|e| e.to_string()))
        } else {
            TopLevel::Suite {
                names: self.scenarios.iter().map(|(n, _)| n.clone()).collect(),
                top_error: top.err().map(|e| e.to_string()),
            }
        }
    }

    fn run_scenario(&mut self, name: &str) -> Result<(), String> {
        let Some((_, body)) = self.scenarios.iter().find(|(n, _)| n == name) else {
            return Err(format!("scenario `{name}` not registered"));
        };
        let body = body.clone();
        run_one(&self.engine, &self.ast, &self.setup, &self.teardown, &body)
    }
}

/// Run one scenario: `setup()` → body → `teardown()` (the latter even on
/// failure). `setup`'s return value is the `ctx` passed to body/teardown.
fn run_one(
    engine: &Engine,
    ast: &AST,
    setup: &Option<FnPtr>,
    teardown: &Option<FnPtr>,
    body: &FnPtr,
) -> Result<(), String> {
    let ctx = match setup {
        Some(s) => s
            .call::<Dynamic>(engine, ast, ())
            .map_err(|e| format!("setup: {e}"))?,
        None => Dynamic::UNIT,
    };
    let result = call_with_ctx(engine, ast, body, ctx.clone());
    if let Some(t) = teardown {
        // Teardown runs regardless; its own error shouldn't mask the body's.
        let _ = call_with_ctx(engine, ast, t, ctx);
    }
    result
}

/// Call a scenario/teardown closure, passing `ctx` only if it takes a parameter
/// (so both `|| …` and `|ctx| …` work).
fn call_with_ctx(engine: &Engine, ast: &AST, f: &FnPtr, ctx: Dynamic) -> Result<(), String> {
    let takes_arg = ast
        .iter_functions()
        .find(|m| m.name == f.fn_name())
        .is_some_and(|m| !m.params.is_empty());
    // `::<Dynamic>` (ignore the return) so a scenario body's last expression may
    // be anything (e.g. a chained assertion) without a type mismatch.
    let res = if takes_arg {
        f.call::<Dynamic>(engine, ast, (ctx,))
    } else {
        f.call::<Dynamic>(engine, ast, ())
    };
    res.map(|_| ()).map_err(|e| e.to_string())
}

/// Names registered via `scenario("name", …)` in a file, for `--scenario` shell
/// completion. Compiles the script (parse only — it never *evaluates*, so it can
/// not start baresip) and walks the AST for `scenario("…")` calls with a string
/// literal name. Returns `[]` if the file can't be read or doesn't parse.
pub fn scenario_names(path: &Path) -> Vec<String> {
    let Ok(src) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(ast) = Engine::new().compile(&src) else {
        return Vec::new();
    };
    // A top-level `scenario("x", …);` parses as `Stmt::FnCall`; nested/expression
    // position uses `Expr::FnCall`. Handle both.
    let mut names = Vec::new();
    let mut collect = |call: &rhai::FnCallExpr| {
        if call.name != "scenario" {
            return;
        }
        if let Some(rhai::Expr::StringConstant(name, _)) = call.args.first() {
            names.push(name.to_string());
        }
    };
    ast.walk(&mut |nodes| {
        match nodes.last() {
            Some(rhai::ASTNode::Stmt(rhai::Stmt::FnCall(call, _))) => collect(call),
            Some(rhai::ASTNode::Expr(rhai::Expr::FnCall(call, _))) => collect(call),
            _ => {}
        }
        true // keep walking
    });
    names
}

/// Whether a directory-discovered file should be run: it registers at least one
/// `scenario(...)`, or it fails to parse (so breakage surfaces as an error rather
/// than being silently skipped). Pure helper/module files — only `fn`s, no
/// scenarios — return `false` and are skipped. Explicitly-named files bypass this.
pub fn dir_should_run(path: &Path) -> bool {
    let Ok(src) = std::fs::read_to_string(path) else {
        return true; // unreadable here → let the run surface the error
    };
    let Ok(ast) = Engine::new().compile(&src) else {
        return true; // doesn't parse → run it so the error is reported, not hidden
    };
    let mut found = false;
    ast.walk(&mut |nodes| {
        let call = match nodes.last() {
            Some(rhai::ASTNode::Stmt(rhai::Stmt::FnCall(c, _))) => Some(c),
            Some(rhai::ASTNode::Expr(rhai::Expr::FnCall(c, _))) => Some(c),
            _ => None,
        };
        if call.is_some_and(|c| c.name == "scenario") {
            found = true;
        }
        !found // stop walking once a scenario call is seen
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_names_from_ast() {
        let dir = std::env::temp_dir().join("ringo_flow_sc_test.rhai");
        std::fs::write(
            &dir,
            "setup(|| #{});\nscenario(\"answered call\", |ctx| {});\n// scenario(\"commented out\", ||{});\nscenario(\"rejected call\", || {});\n",
        )
        .unwrap();
        let names = scenario_names(&dir);
        assert_eq!(names, vec!["answered call", "rejected call"]);
    }

    #[test]
    fn dir_should_run_skips_helpers_keeps_scenarios_and_broken() {
        let write = |name: &str, body: &str| {
            let p = std::env::temp_dir().join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        // a scenario file → run
        assert!(dir_should_run(&write(
            "rf_dsr_scn.rhai",
            "scenario(\"x\", || {});"
        )));
        // a helper/module file (only fns, no scenarios) → skip
        assert!(!dir_should_run(&write(
            "rf_dsr_helper.rhai",
            "fn greet(x) { x }\nlet K = 1;"
        )));
        // a file that doesn't parse → run anyway (so the error is reported)
        assert!(dir_should_run(&write(
            "rf_dsr_broken.rhai",
            "let mut x = 1;"
        )));
    }
}
