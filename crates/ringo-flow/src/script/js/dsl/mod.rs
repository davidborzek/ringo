//! The JS-facing scenario DSL: the `rquickjs` value classes (`Agent`, `Assertion`,
//! `MockServer`, `PathMatch`, `HttpResponse`, `AudioSpec`), their co-located TS type
//! declarations, and the global free functions that back the DSL verbs. Split by
//! domain; `super::bindings::install` wires them in.

pub mod agent;
pub mod assertion;
pub mod audio;
pub mod core;
pub mod http;
pub mod mock;
pub mod scenario;
