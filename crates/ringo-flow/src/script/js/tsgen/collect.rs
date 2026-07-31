//! Collect the submitted model from the [`inventory`] registry into an [`Api`]
//! snapshot the renderer consumes.

use crate::script::js::tsgen::model::{TsDecl, TsInterfaceDef, TsType};

/// A snapshot of the whole collected API, so the renderer operates on plain data rather
/// than reaching into the global inventory. Order is as collected (unspecified); the
/// renderer sorts for a stable layout.
pub struct Api {
    /// Function/method/getter declarations (`#[global]` / `#[export]`).
    pub decls: Vec<&'static TsDecl>,
    /// Raw type declarations (`declare!` / `#[derive(Interface|Enum)]`).
    pub types: Vec<&'static TsType>,
    /// Interface definitions (`#[export]`).
    pub interfaces: Vec<&'static TsInterfaceDef>,
}

impl Api {
    /// Collect the API the `.d.ts` writer needs from the inventory.
    pub fn collect() -> Self {
        Self {
            decls: inventory::iter::<TsDecl>().collect(),
            types: inventory::iter::<TsType>().collect(),
            interfaces: inventory::iter::<TsInterfaceDef>().collect(),
        }
    }
}
