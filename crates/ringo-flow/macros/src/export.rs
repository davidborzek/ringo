//! The `#[export]` (impl → interface) and `#[global]` (free fn → global) bodies.

use crate::{ty, util};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, FnArg, ImplItem, ItemFn, ItemImpl};

/// `#[export]` on `impl T { … }`: emit a `TsDecl` per method/getter under group `T`
/// plus a `TsInterfaceDef`, and re-emit the impl (minus our helper attrs).
pub(crate) fn ts_export(attr: &TokenStream2, mut imp: ItemImpl) -> TokenStream2 {
    if imp.trait_.is_some() {
        return quote!(compile_error!(
            "#[export] does not support trait impls; annotate the inherent `impl` block"
        ););
    }
    // `#[export(generic = "<T>")]` — the interface's generic clause, if any.
    let generic = util::string_arg(attr, "generic").unwrap_or_default();
    let group = ty::type_name(&imp.self_ty);
    let mut submits = TokenStream2::new();
    let mut order: u32 = 0;
    let mut ctor = quote!(None);
    for it in &mut imp.items {
        if let ImplItem::Fn(f) = it {
            if util::has_flag(&f.attrs, "qjs", "constructor") {
                // A `#[qjs(constructor)]` method is the class's `constructor(...)`, not
                // a member — carry its params so the renderer emits a `declare class`.
                match ty::params_ts(&f.sig) {
                    Ok(params) => {
                        let param_toks = params.iter().map(|(n, ts, opt)| {
                            let model = util::model_path();
                            quote!(#model::TsParam { name: #n, ts: #ts, optional: #opt })
                        });
                        ctor = quote!(Some(&[ #(#param_toks),* ]));
                    }
                    Err(e) => return quote!(compile_error!(#e);),
                }
            } else {
                for decl in method_decls(&group, &f.sig, &f.attrs, &mut order) {
                    submits.extend(decl);
                }
            }
            util::strip_ts_attrs(&mut f.attrs);
            for arg in &mut f.sig.inputs {
                if let FnArg::Typed(pt) = arg {
                    util::strip_ts_attrs(&mut pt.attrs);
                }
            }
        }
    }
    let model = util::model_path();
    quote! {
        #imp
        #submits
        ::inventory::submit! {
            #model::TsInterfaceDef { name: #group, generic: #generic, ctor: #ctor }
        }
    }
}

/// `#[global(name = "jsName")]` on a free `fn`: emit a global `TsDecl`, pass the fn
/// through (minus helper attrs).
pub(crate) fn ts_global(attr: &TokenStream2, mut f: ItemFn) -> TokenStream2 {
    let name = util::string_arg(attr, "name").unwrap_or_else(|| f.sig.ident.to_string());
    let generic = util::string_arg(attr, "generic").unwrap_or_default();
    let jd = util::jsdoc(&f.attrs);
    let doc = util::doc_lines(&f.attrs);
    let submit = if jd.sigs.is_empty() {
        let params = match ty::params_ts(&f.sig) {
            Ok(p) => p,
            Err(e) => {
                strip_fn_attrs(&mut f);
                return quote!(#f compile_error!(#e););
            }
        };
        let ret = match jd.ty {
            Some(t) => t,
            None => match ty::return_ts(&f.sig) {
                (inner, true) => format!("{inner} | undefined"),
                (inner, false) => inner,
            },
        };
        submit_tokens(
            "global", &name, "Global", &params, &ret, &doc, &generic, false, 0, "",
        )
    } else {
        // Verbatim global overloads (`#[jsdoc(sig = …)]`): the doc lands on the first.
        let mut ts = TokenStream2::new();
        for (i, s) in jd.sigs.iter().enumerate() {
            let d = if i == 0 { doc.clone() } else { Vec::new() };
            ts.extend(submit_tokens(
                "global",
                &name,
                "Global",
                &[],
                "",
                &d,
                "",
                false,
                i as u32,
                s,
            ));
        }
        ts
    };
    strip_fn_attrs(&mut f);
    quote!(#f #submit)
}

/// Drop our helper `#[jsdoc(...)]` attrs from a free fn and its params.
fn strip_fn_attrs(f: &mut ItemFn) {
    util::strip_ts_attrs(&mut f.attrs);
    for arg in &mut f.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            util::strip_ts_attrs(&mut pt.attrs);
        }
    }
}

/// The `TsDecl`s for one interface method/getter (usually one; several for
/// `#[jsdoc(sig = …)]` overloads), or empty for `#[jsdoc(skip)]`/`#[qjs(skip)]` or a
/// `#[qjs(set)]` setter. Bumps `order` per emitted decl so members keep source order.
fn method_decls(
    group: &str,
    sig: &syn::Signature,
    attrs: &[Attribute],
    order: &mut u32,
) -> Vec<TokenStream2> {
    let jd = util::jsdoc(attrs);
    if util::has_flag(attrs, "qjs", "skip") || jd.skip {
        return Vec::new();
    }
    // A `#[qjs(set)]` setter shares the property the getter already declares.
    if util::has_flag(attrs, "qjs", "set") {
        return Vec::new();
    }
    let doc = util::doc_lines(attrs);
    // Verbatim overload signatures: emit each as a raw member; the doc lands on the first.
    if !jd.sigs.is_empty() {
        return jd
            .sigs
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let d = if i == 0 { doc.clone() } else { Vec::new() };
                let o = *order;
                *order += 1;
                submit_tokens(group, "", "Method", &[], "", &d, "", false, o, s)
            })
            .collect();
    }
    let is_getter = util::has_flag(attrs, "qjs", "get");
    let name = util::qjs_rename(attrs).unwrap_or_else(|| sig.ident.to_string());
    let kind = if is_getter { "Getter" } else { "Method" };
    let params = if is_getter {
        Vec::new()
    } else {
        match ty::params_ts(sig) {
            Ok(p) => p,
            Err(e) => return vec![quote!(compile_error!(#e);)],
        }
    };
    let (ret, ret_opt) = match jd.ty {
        Some(t) => (t, false),
        None => ty::return_ts(sig),
    };
    let optional = jd.optional || (is_getter && ret_opt);
    let ret = if !is_getter && ret_opt {
        format!("{ret} | undefined")
    } else {
        ret
    };
    let o = *order;
    *order += 1;
    vec![submit_tokens(
        group, &name, kind, &params, &ret, &doc, "", optional, o, "",
    )]
}

/// Emit a structured `TsDecl` submission. `raw` is a verbatim member signature (for
/// overloads); empty for the usual derived decl.
#[allow(clippy::too_many_arguments)]
fn submit_tokens(
    group: &str,
    name: &str,
    kind: &str,
    params: &[(String, String, bool)],
    ret: &str,
    doc: &[String],
    generic: &str,
    optional: bool,
    order: u32,
    raw: &str,
) -> TokenStream2 {
    let kind_ident = syn::Ident::new(kind, proc_macro2::Span::call_site());
    let param_toks = params.iter().map(|(n, ts, opt)| {
        let model = util::model_path();
        quote!(#model::TsParam { name: #n, ts: #ts, optional: #opt })
    });
    let model = util::model_path();
    quote! {
        ::inventory::submit! {
            #model::TsDecl {
                group: #group,
                name: #name,
                kind: #model::TsKind::#kind_ident,
                params: &[ #(#param_toks),* ],
                ret: #ret,
                doc: &[ #(#doc),* ],
                generic: #generic,
                optional: #optional,
                order: #order,
                raw: #raw,
            }
        }
    }
}
