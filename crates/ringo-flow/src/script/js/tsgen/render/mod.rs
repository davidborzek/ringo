//! The `.d.ts` writer over an [`Api`](super::collect::Api).

use super::model::TsParam;

mod dts;
pub use dts::dts;

/// Render a parameter list `name: ts, …` (a `?` marks an optional param).
pub(crate) fn params_ts(params: &[TsParam]) -> String {
    params
        .iter()
        .map(|p| format!("{}{}: {}", p.name, if p.optional { "?" } else { "" }, p.ts))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Apply an interface's generic clause to a rendered type fragment: a member whose type
/// names its own interface (e.g. a matcher returning `Assertion`) becomes `Assertion<T>`.
/// No-op when the interface has no generic.
pub(crate) fn apply_generic(iface: &str, generic: &str, s: &str) -> String {
    if generic.is_empty() {
        return s.to_string();
    }
    // Collapse any already-present `Iface<…>` first, so re-application is idempotent.
    let full = format!("{iface}{generic}");
    let collapsed = s.replace(&full, iface);
    // Append the clause to *whole-identifier* `Iface` only (so `Asset` doesn't corrupt
    // `AssetGroup`).
    let mut out = String::new();
    let mut word = String::new();
    let flush = |out: &mut String, word: &mut String| {
        out.push_str(if word == iface { &full } else { word });
        word.clear();
    };
    for ch in collapsed.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            flush(&mut out, &mut word);
            out.push(ch);
        }
    }
    flush(&mut out, &mut word);
    out
}
