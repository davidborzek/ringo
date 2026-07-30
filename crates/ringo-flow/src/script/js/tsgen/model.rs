//! The API model: the data the macros submit (via [`inventory`]) and the `.d.ts`
//! renderer consumes. The stable contract between the proc-macros and the writer.

/// A function/method parameter in the API model.
pub struct TsParam {
    /// The parameter name.
    pub name: &'static str,
    /// The mapped TypeScript type.
    pub ts: &'static str,
    /// `true` if optional (rendered `name?: ts`).
    pub optional: bool,
}

/// What kind of binding a [`TsDecl`] describes.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum TsKind {
    /// A top-level `declare function`.
    Global,
    /// An interface method (`name(params): ret`).
    Method,
    /// An interface `readonly` getter property.
    Getter,
}

/// One API declaration, submitted by the macros and collected via [`inventory`].
pub struct TsDecl {
    /// `"global"` or the interface name (the `impl` type, e.g. `"Agent"`).
    pub group: &'static str,
    /// The JS-visible name (honours `#[qjs(rename = "…")]`).
    pub name: &'static str,
    /// Whether this is a global, an interface method or a getter.
    pub kind: TsKind,
    /// The parameters (empty for a getter).
    pub params: &'static [TsParam],
    /// The mapped TypeScript return type.
    pub ret: &'static str,
    /// The `///` doc lines, in source order.
    pub doc: &'static [&'static str],
    /// A generic clause appended to the name, e.g. `"<T>"`.
    pub generic: &'static str,
    /// `true` for an optional getter (`readonly name?: ret`).
    pub optional: bool,
    /// Source order within the group, so members keep their declared order.
    pub order: u32,
    /// A verbatim member signature (from `#[jsdoc(sig = "…")]`) for an overload that
    /// can't derive from a single Rust signature, e.g. `"respond(path: string |
    /// PathMatch, response: MockResponder): void"`. When non-empty it is rendered as-is
    /// (the structured `name`/`params`/`ret` are unused).
    pub raw: &'static str,
}

inventory::collect!(TsDecl);

/// A raw TypeScript declaration with no runtime binding to derive from (a data shape,
/// `type … = …`, or an authored overload), declared via the `declare!` macro / emitted
/// by `#[derive(Interface)]`/`#[derive(Enum)]`.
pub struct TsType {
    /// The verbatim TS, e.g. `"interface Peer { readonly uri: string; }"`.
    pub decl: &'static str,
}

inventory::collect!(TsType);

/// An interface to render from its derived members: the `#[export]`-annotated type's
/// name and an optional generic clause (`"<T>"`).
pub struct TsInterfaceDef {
    /// The interface name (the annotated type, e.g. `"Agent"`).
    pub name: &'static str,
    /// A generic clause, e.g. `"<T>"`, or `""`.
    pub generic: &'static str,
    /// `Some(params)` → render as a `declare class` with `constructor(params)` (from a
    /// `#[qjs(constructor)]` method); `None` → a plain `interface`.
    pub ctor: Option<&'static [TsParam]>,
}

inventory::collect!(TsInterfaceDef);
