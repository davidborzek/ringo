//! The `#[derive(Interface)]` and `#[derive(Enum)]` bodies.

use crate::{ty, util};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

/// Derive a TS `interface` from a struct (fields → TS, `Option<T>`→optional, `///`
/// docs, `#[jsdoc(...)]` overrides), plus a `FIELDS` const of the JS field names (for
/// unknown-key validation).
pub(crate) fn interface(input: DeriveInput) -> TokenStream2 {
    let ident = &input.ident;
    let name = util::jsdoc(&input.attrs)
        .rename
        .unwrap_or_else(|| ident.to_string());
    let Data::Struct(s) = &input.data else {
        return quote!(compile_error!("Interface only supports structs"););
    };
    let Fields::Named(named) = &s.fields else {
        return quote!(compile_error!("Interface needs named fields"););
    };

    // `#[jsdoc(readonly)]` on the struct → all members `readonly` (for output shapes).
    let ro = if util::jsdoc(&input.attrs).readonly {
        "readonly "
    } else {
        ""
    };
    let mut body = String::new();
    let mut fields: Vec<String> = Vec::new();
    for f in &named.named {
        let jd = util::jsdoc(&f.attrs);
        if jd.skip {
            continue;
        }
        // JS key: explicit `#[jsdoc(rename)]`, else rquickjs's `#[qjs(rename)]`, else the
        // field name.
        let fname = jd
            .rename
            .or_else(|| util::qjs_rename(&f.attrs))
            .unwrap_or_else(|| f.ident.as_ref().unwrap().to_string());
        fields.push(fname.clone());
        let doc = util::doc_lines(&f.attrs);
        for line in &doc {
            body.push_str(&format!("  /** {line} */\n"));
        }
        let (ts, optional) = match jd.ty {
            Some(t) => (t, jd.optional),
            None => ty::map_type(&f.ty),
        };
        let q = if optional { "?" } else { "" };
        body.push_str(&format!("  {ro}{fname}{q}: {ts};\n"));
    }
    let decl = format!("interface {name} {{\n{body}}}");
    let model = util::model_path();

    quote! {
        ::inventory::submit! {
            #model::TsType { decl: #decl }
        }
        impl #ident {
            /// The accepted JS config keys (this interface's field names), for
            /// unknown-key validation. (Unused for output-only shapes.)
            #[allow(dead_code)]
            pub const FIELDS: &'static [&'static str] = &[ #(#fields),* ];
        }
    }
}

/// Derive a TS `declare const enum` from a fieldless enum (variant → lower-cased name,
/// or `#[jsdoc(value = "…")]`), plus a `VALUES` const of `(variant, js_value)` pairs.
pub(crate) fn ts_enum(input: DeriveInput) -> TokenStream2 {
    let ident = &input.ident;
    let name = util::jsdoc(&input.attrs)
        .rename
        .unwrap_or_else(|| ident.to_string());
    let Data::Enum(e) = &input.data else {
        return quote!(compile_error!("Enum only supports enums"););
    };
    let mut members = String::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    for v in &e.variants {
        if !matches!(v.fields, Fields::Unit) {
            return quote!(compile_error!("Enum needs unit (fieldless) variants"););
        }
        let vname = v.ident.to_string();
        let value = util::jsdoc(&v.attrs)
            .value
            .unwrap_or_else(|| vname.to_lowercase());
        members.push_str(&format!(" {vname} = \"{value}\","));
        pairs.push((vname, value));
    }
    let inner = members.trim_end_matches(',');
    let decl = format!("declare const enum {name} {{{inner} }}");
    let names = pairs.iter().map(|(n, _)| n.clone());
    let vals = pairs.iter().map(|(_, v)| v.clone());
    let model = util::model_path();
    quote! {
        ::inventory::submit! {
            #model::TsType { decl: #decl }
        }
        impl #ident {
            /// `(variant_name, js_value)` pairs — build the runtime enum object from these.
            #[allow(dead_code)]
            pub const VALUES: &'static [(&'static str, &'static str)] = &[ #((#names, #vals)),* ];
        }
    }
}
