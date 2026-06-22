//! The language-neutral scenario engine. Knows baresip agents, assertions,
//! HTTP and audio — but nothing about any scripting language. A scripting
//! frontend (e.g. [`crate::script::rhai`]) holds an [`Arc<Ctx>`](ctx::Ctx),
//! exposes thin handles that call these methods, and implements [`ScriptHost`]
//! so [`run`] can drive it. Adding a language touches only `script/`.

pub mod assertion;
pub mod audio;
pub mod ctx;
pub mod duration;
pub mod http;
pub mod mock_server;
mod run;

pub use ctx::{AgentInfo, CallState, Ctx, sip_user_part};
pub use run::{Filters, ScenarioInfo, ScenarioResult, ScriptHost, TopLevel, run};
