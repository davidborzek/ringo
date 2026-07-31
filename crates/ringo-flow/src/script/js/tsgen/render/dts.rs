//! The `.d.ts` writer: a TypeScript declaration file (ambient `interface`s +
//! `declare function`s) from the collected [`Api`].

use super::{apply_generic, params_ts};
use crate::script::js::tsgen::collect::Api;
use crate::script::js::tsgen::model::{TsDecl, TsInterfaceDef, TsKind};
use std::fmt::Write as _;

/// Render the full `.d.ts` from the model: the `declare!`/derived type declarations,
/// one `interface` per `#[export]` type (its methods/getters), then the global
/// `declare function`s. `header` is prepended verbatim (e.g. a "generated" banner).
/// Output is deterministic (stable sort).
pub fn dts(api: &Api, header: &str) -> String {
    let mut out = String::with_capacity(8192);
    if !header.is_empty() {
        out.push_str(header);
        out.push_str("\n\n");
    }

    // Data-shape types (`declare!` / derived interfaces+enums), trimmed and sorted.
    let mut types: Vec<&str> = api.types.iter().map(|t| t.decl.trim()).collect();
    types.sort_unstable();
    for ty in types {
        out.push_str(ty);
        out.push_str("\n\n");
    }

    // Interfaces (sorted by name — order is cosmetic, TS hoists declarations).
    let mut ifaces: Vec<&TsInterfaceDef> = api.interfaces.to_vec();
    ifaces.sort_by_key(|i| i.name);
    // An interface name must come from exactly one source — a `#[ts_export]` impl OR a
    // `#[derive(TsInterface)]` data shape, never both (which emits two `interface X`).
    for iface in &ifaces {
        let dup = api.types.iter().any(|t| {
            t.decl
                .trim_start()
                .strip_prefix("interface ")
                .and_then(|s| s.split([' ', '<', '{']).next())
                == Some(iface.name)
        });
        assert!(
            !dup,
            "interface `{}` declared by both #[ts_export] and a type declaration",
            iface.name
        );
    }
    for iface in ifaces {
        match iface.ctor {
            Some(params) => {
                let _ = writeln!(out, "declare class {}{} {{", iface.name, iface.generic);
                let _ = writeln!(out, "  constructor({});", params_ts(params));
            }
            None => {
                let _ = writeln!(out, "interface {}{} {{", iface.name, iface.generic);
            }
        }
        let mut members: Vec<&TsDecl> = api
            .decls
            .iter()
            .copied()
            .filter(|d| d.group == iface.name)
            .collect();
        members.sort_by(|a, b| {
            a.order
                .cmp(&b.order)
                .then(a.raw.cmp(b.raw))
                .then(a.name.cmp(b.name))
        });
        for d in members {
            render_member(&mut out, d, iface.generic);
        }
        out.push_str("}\n\n");
    }

    // Globals (sorted by name then source order).
    let mut globals: Vec<&TsDecl> = api
        .decls
        .iter()
        .copied()
        .filter(|d| d.group == "global")
        .collect();
    globals.sort_by(|a, b| {
        a.name
            .cmp(b.name)
            .then(a.order.cmp(&b.order))
            .then(a.raw.cmp(b.raw))
    });
    for d in globals {
        render_doc(&mut out, d.doc, "");
        if d.raw.is_empty() {
            let _ = writeln!(
                out,
                "declare function {}{}({}): {};",
                d.name,
                d.generic,
                params_ts(d.params),
                d.ret
            );
        } else {
            let _ = writeln!(out, "declare function {};", d.raw);
        }
    }
    out
}

/// Render one interface member, applying the interface's `generic` to self-referential
/// return/param types.
fn render_member(out: &mut String, d: &TsDecl, generic: &str) {
    render_doc(out, d.doc, "  ");
    if !d.raw.is_empty() {
        let _ = writeln!(out, "  {};", apply_generic(d.group, generic, d.raw));
    } else if d.kind == TsKind::Getter {
        let q = if d.optional { "?" } else { "" };
        let _ = writeln!(
            out,
            "  readonly {}{}: {};",
            d.name,
            q,
            apply_generic(d.group, generic, d.ret)
        );
    } else {
        let _ = writeln!(
            out,
            "  {}({}): {};",
            d.name,
            apply_generic(d.group, generic, &params_ts(d.params)),
            apply_generic(d.group, generic, d.ret)
        );
    }
}

/// Emit a JSDoc block (`/** … */`) at `indent`; a single line stays on one line.
fn render_doc(out: &mut String, doc: &[&str], indent: &str) {
    match doc {
        [] => {}
        [only] => {
            let _ = writeln!(out, "{indent}/** {only} */");
        }
        [first, rest @ ..] => {
            let _ = writeln!(out, "{indent}/** {first}");
            let (last, mid) = rest.split_last().unwrap();
            for line in mid {
                let _ = writeln!(out, "{indent} *  {line}");
            }
            let _ = writeln!(out, "{indent} *  {last} */");
        }
    }
}
