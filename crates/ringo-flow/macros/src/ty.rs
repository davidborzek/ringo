//! Rust→TypeScript type mapping and signature extraction.

use crate::util;
use syn::{FnArg, ReturnType, Type};

/// `(name, ts, optional)` for the JS-visible params, or an `Err` describing a param
/// the generator can't map. Skips `&self`, `#[jsdoc(skip)]` and the injected
/// `Ctx`/`JsCtx`. A reference whose referent a JS caller really passes (`&str`,
/// `&String`, `&[T]`) is **kept**; any other bare reference is an error (the author
/// must `#[jsdoc(skip)]` it or pin a `#[jsdoc(type = "…")]`), never a silent drop.
pub(crate) fn params_ts(sig: &syn::Signature) -> Result<Vec<(String, String, bool)>, String> {
    let mut out = Vec::new();
    for arg in &sig.inputs {
        let FnArg::Typed(pt) = arg else { continue };
        let jd = util::jsdoc(&pt.attrs);
        if jd.skip {
            continue;
        }
        // The injected `Ctx`/`JsCtx` is not a JS arg.
        if is_ctx_arg(&pt.ty) {
            continue;
        }
        // A bare reference that isn't a JS-convertible scalar/slice can't be mapped:
        // error out rather than silently dropping the param from the signature.
        if let Type::Reference(r) = &*pt.ty
            && !is_js_ref(&r.elem)
        {
            return Err(format!(
                "param `{}`: bare reference the .d.ts generator can't map — add \
                 #[jsdoc(skip)] to drop it or #[jsdoc(type = \"…\")] to type it",
                pat_name(pt)
            ));
        }
        let (ts, optional) = match jd.ty {
            Some(t) => (t, jd.optional || is_opt(&pt.ty)),
            None => map_type(&pt.ty),
        };
        out.push((pat_name(pt), ts, optional));
    }
    Ok(out)
}

/// The mapped return type + whether it's optional (`Option<T>`/`Opt<T>` → `undefined`).
/// Wrapped in `Promise<…>` for an `async fn`.
pub(crate) fn return_ts(sig: &syn::Signature) -> (String, bool) {
    let (inner, optional) = match &sig.output {
        ReturnType::Default => ("void".to_string(), false),
        ReturnType::Type(_, ty) => map_type(ty),
    };
    if sig.asyncness.is_some() {
        // Fold optionality inside the Promise: an async `-> Option<T>` is
        // `Promise<T | undefined>`, never `Promise<T> | undefined`.
        let body = if optional {
            format!("{inner} | undefined")
        } else {
            inner
        };
        (format!("Promise<{body}>"), false)
    } else {
        (inner, optional)
    }
}

/// Map a Rust type to a TS type string + whether it is optional (`Opt<T>`/`Option<T>`).
pub(crate) fn map_type(ty: &Type) -> (String, bool) {
    // References are transparent: a JS string arrives as `&str`, a byte slice as `&[u8]`.
    if let Type::Reference(r) = ty {
        return map_type(&r.elem);
    }
    if let Type::Slice(s) = ty {
        return (format!("{}[]", map_type(&s.elem).0), false);
    }
    if let Type::Array(a) = ty {
        return (format!("{}[]", map_type(&a.elem).0), false);
    }
    // The unit type `()` (e.g. an unwrapped `JsResult<()>`) is `void`.
    if let Type::Tuple(t) = ty
        && t.elems.is_empty()
    {
        return ("void".to_string(), false);
    }
    // rquickjs `Opt<T>` (fn args) and std `Option<T>` (struct fields) → optional.
    if let Some(inner) = generic_inner(ty, "Opt").or_else(|| generic_inner(ty, "Option")) {
        return (map_type(&inner).0, true);
    }
    // Unwrap `Result`/`JsResult`, keeping the value's optionality.
    for w in ["JsResult", "Result"] {
        if let Some(inner) = generic_inner(ty, w) {
            return map_type(&inner);
        }
    }
    // Transparent wrappers (smart pointers / rquickjs coercion) → their inner type.
    for w in ["Box", "Rc", "Arc", "Cow", "Coerced", "Persistent"] {
        if let Some(inner) = generic_inner(ty, w) {
            return map_type(&inner);
        }
    }
    if let Some(inner) = generic_inner(ty, "Vec") {
        return (format!("{}[]", map_type(&inner).0), false);
    }
    for m in ["HashMap", "BTreeMap", "IndexMap"] {
        if let Some((k, v)) = generic_pair(ty, m) {
            return (
                format!("Record<{}, {}>", map_type(&k).0, map_type(&v).0),
                false,
            );
        }
    }
    if let Some(inner) = generic_inner(ty, "Class") {
        // `Class<'js, X>` → the X type name (its JS class name usually matches).
        return (type_name(&inner), false);
    }
    if let Some(inner) = generic_inner(ty, "Promise") {
        return (format!("Promise<{}>", map_type(&inner).0), false);
    }
    let n = type_name(ty);
    let ts = match n.as_str() {
        "String" | "str" => "string",
        "bool" => "boolean",
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" | "f32"
        | "f64" => "number",
        "BigInt" => "bigint",
        "Value" => "any",
        "Object" => "object",
        "Array" => "any[]",
        "Null" => "null",
        "Undefined" => "undefined",
        "Function" => "Function",
        // An unrecognised name is emitted **verbatim** — correct for our own
        // `#[derive(Interface)]`/`#[derive(Enum)]` types (TS name == Rust name); a std
        // type with no TS form (e.g. `Duration`) yields an undeclared name — pin it with
        // `#[jsdoc(type = "…")]`.
        other => other,
    };
    (ts.to_string(), false)
}

/// `true` for the implicit `Ctx<'js>` / `JsCtx<'js>` rquickjs context argument.
fn is_ctx_arg(ty: &Type) -> bool {
    matches!(type_name(ty).as_str(), "Ctx" | "JsCtx")
}

/// `true` for a reference whose referent a JS caller really passes (`&str`, `&String`,
/// `&[T]`) — as opposed to captured host state (`&Arc<…>`).
fn is_js_ref(inner: &Type) -> bool {
    matches!(inner, Type::Slice(_)) || matches!(type_name(inner).as_str(), "str" | "String")
}

fn is_opt(ty: &Type) -> bool {
    generic_inner(ty, "Opt").is_some() || generic_inner(ty, "Option").is_some()
}

/// The last path segment's identifier of a (possibly referenced) type.
pub(crate) fn type_name(ty: &Type) -> String {
    if let Type::Path(p) = ty
        && let Some(seg) = p.path.segments.last()
    {
        return seg.ident.to_string();
    }
    if let Type::Reference(r) = ty {
        return type_name(&r.elem);
    }
    "any".to_string()
}

/// The parameter's binding name (`pat: ty` → `pat`); falls back to `arg`.
fn pat_name(pt: &syn::PatType) -> String {
    if let syn::Pat::Ident(i) = &*pt.pat {
        i.ident.to_string()
    } else {
        "arg".to_string()
    }
}

/// The single generic argument of `Wrapper<T>` (by last-segment name), or `None`.
fn generic_inner(ty: &Type, wrapper: &str) -> Option<Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != wrapper {
        return None;
    }
    if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
        for a in &ab.args {
            if let syn::GenericArgument::Type(t) = a {
                return Some(t.clone());
            }
        }
    }
    None
}

/// The two generic type arguments of `Map<K, V>` (ignoring any lifetime/hasher args).
fn generic_pair(ty: &Type, wrapper: &str) -> Option<(Type, Type)> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != wrapper {
        return None;
    }
    if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
        let tys: Vec<Type> = ab
            .args
            .iter()
            .filter_map(|a| {
                if let syn::GenericArgument::Type(t) = a {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .collect();
        if tys.len() >= 2 {
            return Some((tys[0].clone(), tys[1].clone()));
        }
    }
    None
}
