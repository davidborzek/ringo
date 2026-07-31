//! Attribute / token helpers shared across the macros: reading `///` docs and
//! `#[jsdoc(...)]` / `#[qjs(...)]` args and flags.

use syn::{Attribute, Lit, Meta};

/// The consumer-crate path to the tsgen model that the emitted `inventory::submit!`s
/// reference. Centralized so the coupling to `ringo-flow`'s module layout lives in one
/// place; the consumer MUST expose `TsDecl`/`TsParam`/`TsKind`/`TsType`/`TsInterfaceDef`
/// at this path.
pub(crate) fn model_path() -> proc_macro2::TokenStream {
    quote::quote!(crate::script::js::tsgen)
}

/// Whether an attribute's last path segment is `name` (matches bare `#[name]` and
/// fully-qualified `#[crate::name]`).
pub(crate) fn is_attr(a: &Attribute, name: &str) -> bool {
    a.path().segments.last().is_some_and(|s| s.ident == name)
}

/// The `///` doc lines on an item, trimmed, in source order.
pub(crate) fn doc_lines(attrs: &[Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for a in attrs {
        if a.path().is_ident("doc")
            && let Meta::NameValue(nv) = &a.meta
            && let syn::Expr::Lit(el) = &nv.value
            && let Lit::Str(s) = &el.lit
        {
            out.push(s.value().trim().to_string());
        }
    }
    out
}

/// The parsed `#[jsdoc(...)]` metadata on an item: `type = "…"` (the TS type override),
/// `rename = "…"`, `value = "…"` (enum variant), and the `optional` / `skip` /
/// `readonly` flags — all combinable (`#[jsdoc(type = "Peer", optional)]`).
#[derive(Default)]
pub(crate) struct JsDoc {
    pub ty: Option<String>,
    pub rename: Option<String>,
    pub value: Option<String>,
    /// Verbatim member signatures (repeatable `sig = "…"`) for overloads that can't
    /// derive from a single Rust signature; replaces the derived signature.
    pub sigs: Vec<String>,
    pub optional: bool,
    pub skip: bool,
    pub readonly: bool,
}

impl syn::parse::Parse for JsDoc {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut d = JsDoc::default();
        while !input.is_empty() {
            if input.peek(syn::Token![type]) {
                input.parse::<syn::Token![type]>()?;
                input.parse::<syn::Token![=]>()?;
                d.ty = Some(input.parse::<syn::LitStr>()?.value());
            } else {
                let key: syn::Ident = input.parse()?;
                if input.peek(syn::Token![=]) {
                    input.parse::<syn::Token![=]>()?;
                    let v = input.parse::<syn::LitStr>()?.value();
                    match key.to_string().as_str() {
                        "rename" => d.rename = Some(v),
                        "value" => d.value = Some(v),
                        "sig" => d.sigs.push(v),
                        other => {
                            return Err(syn::Error::new(
                                key.span(),
                                format!("unknown jsdoc key `{other}`"),
                            ));
                        }
                    }
                } else {
                    match key.to_string().as_str() {
                        "optional" => d.optional = true,
                        "skip" => d.skip = true,
                        "readonly" => d.readonly = true,
                        other => {
                            return Err(syn::Error::new(
                                key.span(),
                                format!("unknown jsdoc flag `{other}`"),
                            ));
                        }
                    }
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<syn::Token![,]>()?;
        }
        Ok(d)
    }
}

/// Merge all `#[jsdoc(...)]` attributes on an item into one [`JsDoc`].
pub(crate) fn jsdoc(attrs: &[Attribute]) -> JsDoc {
    let mut out = JsDoc::default();
    for a in attrs {
        if a.path().is_ident("jsdoc")
            && let Ok(d) = a.parse_args::<JsDoc>()
        {
            out.ty = d.ty.or(out.ty);
            out.rename = d.rename.or(out.rename);
            out.value = d.value.or(out.value);
            out.sigs.extend(d.sigs);
            out.optional |= d.optional;
            out.skip |= d.skip;
            out.readonly |= d.readonly;
        }
    }
    out
}

/// `#[qjs(rename = "…")]` — the JS-visible name rquickjs uses.
pub(crate) fn qjs_rename(attrs: &[Attribute]) -> Option<String> {
    for a in attrs {
        if a.path().is_ident("qjs") {
            let mut found = None;
            let _ = a.parse_nested_meta(|m| {
                if m.path.is_ident("rename") {
                    found = Some(m.value()?.parse::<syn::LitStr>()?.value());
                }
                Ok(())
            });
            if found.is_some() {
                return found;
            }
        }
    }
    None
}

/// Whether `#[ns(flag)]` is present (e.g. `#[qjs(get)]`, `#[qjs(skip)]`).
pub(crate) fn has_flag(attrs: &[Attribute], ns: &str, flag: &str) -> bool {
    for a in attrs {
        if a.path().is_ident(ns) {
            let mut hit = false;
            let _ = a.parse_nested_meta(|m| {
                if m.path.is_ident(flag) {
                    hit = true;
                }
                Ok(())
            });
            if hit {
                return true;
            }
        }
    }
    false
}

/// Drop our `#[jsdoc(...)]` helper attrs so rustc and `#[rquickjs::methods]` don't choke.
pub(crate) fn strip_ts_attrs(attrs: &mut Vec<Attribute>) {
    attrs.retain(|a| !is_attr(a, "jsdoc"));
}

/// A `key = "value"` string argument in an attribute arg list (e.g.
/// `#[global(name = "expect", generic = "<T>")]`).
pub(crate) fn string_arg(attr: &proc_macro2::TokenStream, key: &str) -> Option<String> {
    use syn::punctuated::Punctuated;
    let metas = syn::parse::Parser::parse2(
        Punctuated::<Meta, syn::Token![,]>::parse_terminated,
        attr.clone(),
    )
    .unwrap_or_default();
    for m in metas {
        if let Meta::NameValue(nv) = &m
            && nv.path.is_ident(key)
            && let syn::Expr::Lit(el) = &nv.value
            && let Lit::Str(s) = &el.lit
        {
            return Some(s.value());
        }
    }
    None
}
