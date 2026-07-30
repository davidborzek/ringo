//! Compile-time TypeScript metadata extraction for ringo-flow's [`rquickjs`] bindings.
//!
//! `#[export]` on an inherent `impl` (placed *above* `#[rquickjs::methods]`) and
//! `#[global(name = "…")]` on a free function read the Rust signature (param names,
//! arity, optionality, `async`), the `///` docs and any `#[jsdoc(...)]` overrides, and
//! emit an `inventory::submit!` the consumer renders into a `.d.ts`. `#[derive(Interface)]`
//! / `#[derive(Enum)]` do the same for data structs / fieldless enums. The original item
//! is re-emitted unchanged (minus `#[jsdoc(...)]`), so `#[rquickjs::methods]` still runs.
//!
//! Internal to ringo-flow — unstable, ungeneric, tailored to ringo's bindings.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use syn::{DeriveInput, ItemFn, ItemImpl, parse_macro_input};

mod derive;
mod export;
mod ty;
mod util;

/// `#[derive(TsInterface)]` on a struct → a TS `interface` + a `FIELDS` const of the
/// field names.
#[proc_macro_derive(TsInterface, attributes(jsdoc))]
pub fn derive_interface(item: TokenStream) -> TokenStream {
    derive::interface(parse_macro_input!(item as DeriveInput)).into()
}

/// `#[derive(TsEnum)]` on a fieldless enum → a TS `declare const enum` + a `VALUES`
/// const of `(variant, js_value)` pairs.
#[proc_macro_derive(TsEnum, attributes(jsdoc))]
pub fn derive_enum(item: TokenStream) -> TokenStream {
    derive::ts_enum(parse_macro_input!(item as DeriveInput)).into()
}

/// `#[ts_export]` on `impl T { … }`: emit a `TsDecl` per method/getter under group `T`.
#[proc_macro_attribute]
pub fn ts_export(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = TokenStream2::from(attr);
    let imp = parse_macro_input!(item as ItemImpl);
    export::ts_export(&attr, imp).into()
}

/// `#[ts_global(name = "jsName")]` on a free `fn`: emit a global `TsDecl`.
#[proc_macro_attribute]
pub fn ts_global(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = TokenStream2::from(attr);
    let f = parse_macro_input!(item as ItemFn);
    export::ts_global(&attr, f).into()
}
