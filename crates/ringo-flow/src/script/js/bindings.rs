//! Wire the scenario DSL into the QuickJS global object: `scenario`, `agent`,
//! `expect`, `until`, `log`, `env`, plus the `State` enum. Each global is a free Rust
//! fn (or a small closure) registered with `Function::new`; none capture host state —
//! they read the engine/registry/env from the context userdata ([`HostState`], stored
//! here), so registration is uniform and the sync and async globals work the same way.

use super::dsl::agent::Agent;
use super::dsl::assertion::expect_global;
use super::dsl::audio::{audio_file, audio_silence, audio_tone, verify_audio_connection_async};
use super::dsl::core::{
    HostState, default_timeout_global, env_global, load_env_global, log_global, until_async,
    uuid_global, wait_async,
};
use super::dsl::http::http_async;
use super::dsl::mock::{MockServer, json_response_global, regex_global, text_response_global};
use super::dsl::scenario::{register_scenario, setup_global, skip_global, teardown_global};
use super::host::{EnvVars, Registry};
use crate::engine::ctx::Ctx as EngineCtx;
use rquickjs::function::{Async, Opt};
use rquickjs::{Class, Ctx, Function, Object, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Install every global into `ctx`'s global object. `env`/`overrides`/`base_dir` wire
/// `env(...)`, `--set` globals and `loadEnv(...)` (paths resolve against the scenario's
/// directory).
pub fn install(
    ctx: &Ctx<'_>,
    engine: &Arc<EngineCtx>,
    registry: &Arc<Registry>,
    env: &EnvVars,
    overrides: &HashMap<String, String>,
    base_dir: &Path,
) -> rquickjs::Result<()> {
    let g = ctx.globals();

    // Shared host state every global reads through the context userdata — stored before
    // any global can be called, so none of them need to capture it.
    let _ = ctx.store_userdata(HostState {
        eng: engine.clone(),
        reg: registry.clone(),
        env: env.clone(),
        base: base_dir.to_path_buf(),
    });

    /// Register a function-valued global by its JS name, wrapping the
    /// `g.set(name, Function::new(ctx.clone(), …)?)?` ceremony. Uses the enclosing
    /// `install`'s `g`/`ctx` (a `macro_rules!` in a fn may).
    macro_rules! set {
        ($name:literal, $f:expr) => {
            g.set($name, Function::new(ctx.clone(), $f)?)?
        };
    }

    // `--set key=value` overrides become string globals (mirrors rhai pushing them as
    // scope constants), so the script can reference them by bare name.
    for (key, value) in overrides {
        g.set(key.as_str(), value.as_str())?;
    }

    // Scenario lifecycle. (`scenario` stays a closure — `register_scenario` has two
    // independent arg lifetimes, written as the two inferred `'_` here.)
    set!("scenario", |name: String,
                      a: Value<'_>,
                      b: Opt<Function<'_>>|
     -> rquickjs::Result<()> {
        register_scenario(name, a, b)
    });
    // `scenario.each(table)(name, opts?, body)` — parametrised scenarios. The shim
    // attaches `.each` to the `scenario` global (set above) by evaluating a small
    // IIFE that closes over it. Must run after `scenario` is registered.
    ctx.eval::<(), _>(super::dsl::scenario::EACH_SHIM)?;
    set!("skip", skip_global);
    set!("setup", setup_global);
    set!("teardown", teardown_global);

    // Audio sources for `sendAudio`.
    set!("tone", audio_tone);
    set!("file", audio_file);
    set!("silence", audio_silence);

    // HTTP client + mock server.
    set!("http", Async(http_async));
    set!(
        "verifyAudioConnection",
        Async(verify_audio_connection_async)
    );
    set!("regex", regex_global);
    set!("jsonResponse", json_response_global);
    set!("textResponse", text_response_global);
    Class::<MockServer>::define(&g)?;

    // Agents + assertions.
    Class::<Agent>::define(&g)?;
    set!("expect", expect_global);

    // Flow, timing, environment, utilities.
    set!("until", Async(until_async));
    set!("log", log_global);
    set!("env", env_global);
    set!("loadEnv", load_env_global);
    set!("defaultTimeout", default_timeout_global);
    set!("wait", Async(wait_async));
    set!("uuid", uuid_global);

    // State enum: `{ Idle: "idle", … }` — an object, not a function, so set directly.
    g.set("State", super::dsl::agent::State::object(ctx)?)?;

    Ok(())
}

/// Error if `config` carries a key outside `allowed` (catches typos like `passwrod`),
/// mirroring the rhai frontend's `reject_unknown_keys`. `label` prefixes the error.
pub(super) fn reject_unknown_keys(
    label: &str,
    config: &Object<'_>,
    allowed: &[&str],
) -> Result<(), String> {
    for key in config.keys::<String>().flatten() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{label}: unknown config key `{key}`"));
        }
    }
    Ok(())
}
