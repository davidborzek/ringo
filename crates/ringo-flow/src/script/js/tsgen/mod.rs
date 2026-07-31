//! Internal codegen for the JS scenario API: the data model the `ringo-flow-macros`
//! attributes/derives submit (via [`inventory`]) and the `.d.ts` writer that consumes
//! it. Interface methods/getters and globals are derived from the Rust binding
//! signatures by `#[export]` / `#[global]`; data shapes by `#[derive(Interface|Enum)]`
//! or declared verbatim with [`declare!`].
//!
//! Internal to ringo-flow — unstable, ungeneric, tailored to ringo's bindings.

mod collect;
mod model;
mod render;

pub use collect::Api;
pub use model::{TsDecl, TsInterfaceDef, TsKind, TsParam, TsType};
pub use render::dts;

/// Declare a raw TypeScript type/interface next to the domain code it documents; it is
/// collected into the rendered `.d.ts`. For data shapes with no rquickjs binding to
/// derive from (`type … = …`, authored overloads). Emitted **verbatim** — include the
/// trailing punctuation yourself.
macro_rules! declare {
    ($decl:literal) => {
        ::inventory::submit! {
            $crate::script::js::tsgen::TsType { decl: $decl }
        }
    };
}
pub(crate) use declare;

#[cfg(test)]
mod tests {
    use super::{Api, dts};

    /// The committed `.d.ts` must equal what the bindings generate — so a binding change
    /// without a regenerate is caught in CI rather than shipping a stale `.d.ts`.
    #[test]
    fn committed_dts_is_up_to_date() {
        let committed = include_str!("../../../../../../docs/src/ringo-flow/ringo-flow.d.ts");
        let generated = dts(&Api::collect(), super::super::DTS_HEADER);
        assert_eq!(
            generated, committed,
            "ringo-flow.d.ts is stale — regenerate with \
             `ringo-flow definitions --lang js`",
        );
    }
}
