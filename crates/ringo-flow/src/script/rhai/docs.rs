//! Generate the scenario API reference from the registered Rhai functions: build a
//! doc-only engine, read its function metadata, and render the mdBook API pages
//! (one Markdown page per section), the `.d.rhai` definitions and the HTML output.
//! Pure derivation from the `///` doc comments on the `reg!` registrations.

use super::{bindings, host};
use crate::engine::Ctx;
use anyhow::{Context, Result};
use rhai::Engine;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Build an engine purely to enumerate the registered API (definitions/docs). No
/// baresip is started; the throwaway `Ctx`'s verbs are never called. The runtime
/// is returned so its `Handle` (held by `Ctx`) stays valid.
fn doc_engine() -> Result<(tokio::runtime::Runtime, Engine)> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let ctx = Arc::new(Ctx::new(
        rt.handle().clone(),
        Box::new(crate::runtime::report::Json),
        super::DEFAULT_TIMEOUT,
    ));
    let engine = bindings::build_engine(
        ctx,
        Arc::new(host::Registry::default()),
        Arc::default(),
        PathBuf::from("."),
    );
    Ok((rt, engine))
}

/// Write a Rhai definition file (`.d.rhai`) describing the whole scenario API
/// (functions, getters, types, the `State` namespace) for the Rhai language
/// server (completion/hover).
pub fn write_definitions(out: &Path) -> Result<()> {
    let (_rt, engine) = doc_engine()?;
    let scope = rhai::Scope::new();
    engine
        .definitions_with_scope(&scope)
        .write_to_file(out)
        .with_context(|| format!("write {}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

/// Types whose first parameter is the call target — rendered as a method/getter on
/// a receiver (`agent.dial(…)`, `agent.registered`) rather than a free function.
const RECEIVERS: &[&str] = &[
    "Agent",
    "Peer",
    "CallQuality",
    "Assertion",
    "HttpResponse",
    "HttpMock",
    "MockRequest",
    "PathPattern",
    "AudioSpec",
];

/// One documented API entry (a single overload): its call form, receiver/return
/// types, description and any examples (from `# Example` ```rhai blocks in the doc
/// comment). Overloads are separate entries, each with its own heading.
#[derive(Default)]
struct Entry {
    /// The receiver type for a method/getter (`HttpResponse`, `Agent`, …), so the
    /// docs can state what the call is made on; `None` for free functions.
    receiver: Option<String>,
    sigs: Vec<String>,
    returns: Option<String>,
    /// A documented return type from a `# Returns: <type>` doc-comment line, used
    /// for the badge instead of the Rhai metadata type. Lets a dynamic (`?`) return
    /// read as its real shape (e.g. `string?`) without putting non-Rhai syntax in
    /// the `.d.rhai`, which keeps `params_info` as `?`.
    doc_returns: Option<String>,
    summaries: Vec<String>,
    examples: Vec<String>,
}

/// The API index: `(section, rank, display-name) -> Entry`. The key orders
/// sections, then by rank (a type's constructor first, `0`, then its members `1`),
/// then names alphabetically.
type ApiIndex = std::collections::BTreeMap<((u8, &'static str), u8, String), Entry>;

/// Parse the API into an [`ApiIndex`], merging overloads. Functions without doc
/// comments (operators, the Rhai stdlib) are skipped.
fn api_entries(engine: &Engine) -> Result<ApiIndex> {
    let json = engine
        .gen_fn_metadata_to_json(false)
        .context("generate function metadata")?;
    let meta: serde_json::Value = serde_json::from_str(&json).context("parse metadata JSON")?;
    let mut map: ApiIndex = std::collections::BTreeMap::new();

    // rhai's metadata order isn't stable; sort by signature so the merged overload
    // order (and thus the rendered output) is deterministic across runs.
    let mut funcs: Vec<&serde_json::Value> =
        meta["functions"].as_array().into_iter().flatten().collect();
    funcs.sort_by_key(|f| f["signature"].as_str().unwrap_or("").to_string());

    for f in funcs {
        let comments: Vec<&str> = f["docComments"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| d.as_str())
            .collect();
        if comments.is_empty() {
            continue;
        }
        let sig = f["signature"].as_str().unwrap_or("");
        if sig.is_empty() {
            continue;
        }
        let (name, params, ret) = parse_sig(sig);
        if name == "to_string" {
            continue; // a printing helper, not part of the scenario vocabulary
        }
        let receiver = params
            .first()
            .and_then(|p| p.split_once(": "))
            .map(|(_, t)| t.trim())
            .filter(|t| RECEIVERS.contains(t));
        let (summary, examples, doc_ret) = parse_doc(&comments);

        // Rank within a section, which also groups the page: 0 = the constructor of
        // the section's main type (a free function returning Agent/Assertion/
        // HttpResponse/HttpMock, e.g. `agent`/`http`/`mock_server`); 1 = a method (a
        // receiver verb); 2 = a field (a getter, `get$…`, read as `recv.prop`);
        // 3 = a helper / standalone free function (`regex`, `json_response`, globals).
        let primary = matches!(
            ret.as_deref(),
            Some("Agent" | "Assertion" | "HttpResponse" | "HttpMock")
        );
        let (sec, rank) = if receiver.is_some() {
            let kind = if name.starts_with("get$") { 2 } else { 1 };
            (section(receiver, &name), kind)
        } else if primary {
            (entity_section(ret.as_deref().unwrap()).unwrap(), 0)
        } else {
            (section(None, &name), 3)
        };
        // Key by the call form so each overload is its own entry (its own heading and
        // sidebar/TOC line), with its own description and example.
        let cf = call_form(&name, receiver, &params);
        let e = map.entry((sec, rank, cf.clone())).or_default();
        if !e.sigs.contains(&cf) {
            e.sigs.push(cf);
        }
        if e.receiver.is_none() {
            e.receiver = receiver.map(String::from);
        }
        if e.returns.is_none() {
            e.returns = ret;
        }
        if e.doc_returns.is_none() {
            e.doc_returns = doc_ret;
        }
        if !summary.is_empty() && !e.summaries.contains(&summary) {
            e.summaries.push(summary);
        }
        for ex in examples {
            if !e.examples.contains(&ex) {
                e.examples.push(ex);
            }
        }
    }
    Ok(map)
}

/// Split a signature `name(p: T, …) [-> Ret]` into `(name, params, return)`. Our
/// parameter types contain no commas, so a flat split is safe.
fn parse_sig(sig: &str) -> (String, Vec<String>, Option<String>) {
    let (head, ret) = match sig.split_once(" -> ") {
        Some((h, r)) => (h, Some(r.trim().to_string())),
        None => (sig, None),
    };
    let (name, params) = match head.split_once('(') {
        Some((n, rest)) => {
            let rest = rest.trim_end().trim_end_matches(')');
            let params = if rest.trim().is_empty() {
                Vec::new()
            } else {
                rest.split(',').map(|p| p.trim().to_string()).collect()
            };
            (n.trim().to_string(), params)
        }
        None => (head.trim().to_string(), Vec::new()),
    };
    (name, params, ret)
}

/// The `.rhai` call form shown in the signature block, making the receiver explicit
/// so it's clear these are method/getter calls: a getter as `recv.prop`, a method as
/// `recv.name(args)` (e.g. `assertion.equals(expected)`, `resp.json(path)`), and a
/// free function as `name(args)`.
fn call_form(name: &str, receiver: Option<&str>, params: &[String]) -> String {
    if let Some(prop) = name.strip_prefix("get$") {
        let recv = receiver.map(recv_var).unwrap_or("value");
        format!("{recv}.{prop}")
    } else if let Some(r) = receiver {
        format!(
            "{}.{name}({})",
            recv_var(r),
            params.get(1..).unwrap_or(&[]).join(", ")
        )
    } else {
        format!("{name}({})", params.join(", "))
    }
}

/// A readable variable name for a receiver type, for getter call forms.
fn recv_var(ty: &str) -> &'static str {
    match ty {
        "Agent" => "agent",
        "Peer" => "peer",
        "CallQuality" => "quality",
        "Assertion" => "assertion",
        "HttpResponse" => "resp",
        "HttpMock" => "mock",
        "MockRequest" => "req",
        "AudioSpec" => "spec",
        "PathPattern" => "pattern",
        _ => "value",
    }
}

/// The reference section for a function, from its receiver type and name.
/// `(order, title)`.
fn section(receiver: Option<&str>, name: &str) -> (u8, &'static str) {
    let base = name.strip_prefix("get$").unwrap_or(name);
    match receiver {
        // Audio verbs (`agent.send_audio`/`verify_audio`/…) are Agent methods, so
        // they live in Agents; the `tone`/`file`/`silent` sources that build their
        // `AudioSpec` argument get their own AudioSpec section (matched below).
        Some("Agent") => (2, "Agents"),
        // Sub-types reached through another type's member (or, for AudioSpec, used as
        // an argument) get their own section, nested under the parent in SUMMARY.md:
        // `Peer` via `agent.peer`, `MockRequest` via `mock.last_request()`,
        // `AudioSpec` via the `agent.send_audio` argument.
        Some("Peer") => (3, "Peer"),
        Some("CallQuality") => (3, "CallQuality"),
        _ if matches!(base, "tone" | "file" | "silent") => (4, "AudioSpec"),
        Some("Assertion") => (5, "Assertions and matchers"),
        Some("HttpResponse") => (6, "HTTP"),
        Some("HttpMock") | Some("PathPattern") => (7, "HTTP mock server"),
        Some("MockRequest") => (8, "Mock request"),
        _ if matches!(
            base,
            "mock_server" | "json_response" | "text_response" | "regex"
        ) =>
        {
            (7, "HTTP mock server")
        }
        _ if base == "http" => (6, "HTTP"),
        // Receiver-less globals, grouped semantically (instead of one "Top-level").
        _ if matches!(base, "scenario" | "setup" | "teardown" | "skip") => {
            (0, "Scenario structure")
        }
        _ if matches!(
            base,
            "await_until" | "wait" | "parallel" | "default_timeout"
        ) =>
        {
            (1, "Flow and timing")
        }
        _ if matches!(base, "env" | "load_env") => (9, "Environment"),
        _ => (10, "Utilities"), // log, uuid, and any future global
    }
}

/// The section a type's constructor belongs to (keyed by its return type), so
/// `agent`/`assert`/`http`/`mock_server`/`regex` lead their type's section instead
/// of sitting in a generic list. `None` for non-entity return types.
fn entity_section(ret: &str) -> Option<(u8, &'static str)> {
    match ret {
        "Agent" => Some((2, "Agents")),
        "Peer" => Some((3, "Peer")),
        "Assertion" => Some((5, "Assertions and matchers")),
        "HttpResponse" => Some((6, "HTTP")),
        "HttpMock" | "PathPattern" => Some((7, "HTTP mock server")),
        "MockRequest" => Some((8, "Mock request")),
        _ => None,
    }
}

/// Split doc comments into a description, the `# Example` ```rhai code blocks and
/// an optional return type. Fenced blocks become examples (rendered as code); a
/// `# Returns: <type>` line sets the documented return type (e.g. `string?`) and is
/// dropped from the description; a lone `Example` heading is dropped (we render our
/// own label); the rest is the description.
fn parse_doc(comments: &[&str]) -> (String, Vec<String>, Option<String>) {
    let lines: Vec<String> = comments
        .iter()
        .flat_map(|c| c.lines())
        .map(strip_marker)
        .collect();
    let mut summary: Vec<String> = Vec::new();
    let mut examples: Vec<String> = Vec::new();
    let mut doc_returns: Option<String> = None;
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim_start().starts_with("```") {
            let mut block: Vec<String> = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                block.push(lines[i].clone());
                i += 1;
            }
            i += 1; // closing fence
            examples.push(block.join("\n").trim_end().to_string());
        } else {
            let t = lines[i].trim();
            let bare = t.trim_start_matches('#').trim();
            if let Some(ret) = bare
                .strip_prefix("Returns:")
                .or_else(|| bare.strip_prefix("returns:"))
            {
                doc_returns = Some(ret.trim().to_string());
            } else if !bare.eq_ignore_ascii_case("example") {
                summary.push(lines[i].clone());
            }
            i += 1;
        }
    }
    (summary.join("\n").trim().to_string(), examples, doc_returns)
}

/// Strip a doc-comment marker (`///`, `/**`, `*/`, leading `*`) and one space,
/// preserving the rest (so example indentation survives).
fn strip_marker(line: &str) -> String {
    let t = line.trim_start();
    let t = t
        .strip_prefix("///")
        .or_else(|| t.strip_prefix("/**"))
        .unwrap_or(t);
    let t = t.strip_suffix("*/").unwrap_or(t);
    let t = t.strip_prefix('*').unwrap_or(t);
    t.strip_prefix(' ').unwrap_or(t).to_string()
}

/// A GitHub/mdBook-style heading slug (lowercase, non-alphanumerics → hyphens).
fn slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// Map a return type for display (`?` → `any`).
fn ret_display(ret: &str) -> &str {
    if ret == "?" { "any" } else { ret }
}

/// The book page (within `api/`) documenting a type, for cross-linking receiver /
/// return types. `None` for primitives (`int`, `string`, …) and untyped values.
fn type_page(ty: &str) -> Option<&'static str> {
    Some(match ty {
        "Agent" => "agents.md",
        "Peer" => "peer.md",
        "Assertion" => "assertions-and-matchers.md",
        "HttpResponse" => "http.md",
        "HttpMock" => "http-mock-server.md",
        "MockRequest" => "mock-request.md",
        // PathPattern is built by `regex`; link to that entry, not the page top.
        "PathPattern" => "http-mock-server.md#regex",
        "AudioSpec" => "audiospec.md",
        "CallState" => "call-state.md",
        _ => return None,
    })
}

/// Render a type for the Receiver/Returns line: a link to its page if it has one
/// (so `Returns Peer` jumps to the Peer section), otherwise inline code.
fn type_md(ty: &str) -> String {
    match type_page(ty) {
        Some(page) => format!("[`{ty}`]({page})"),
        None => format!("`{ty}`"),
    }
}

/// A regex over the documented type names, for cross-linking them inside signatures.
fn type_re() -> regex::Regex {
    regex::Regex::new(
        r"\b(Agent|Peer|Assertion|HttpResponse|HttpMock|MockRequest|PathPattern|AudioSpec)\b",
    )
    .expect("valid type regex")
}

/// The documented type names referenced in a signature, in order, deduped — for
/// the **Takes** line (the heading itself stays plain text, since links in an mdBook
/// heading break its on-page TOC).
fn sig_types(sig: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for m in type_re().find_iter(sig) {
        if !out.contains(&m.as_str()) {
            out.push(m.as_str());
        }
    }
    out
}

/// The stable anchor id for an entry — the bare verb/getter name (drop a leading
/// `recv.` and the `(params)`), so the guide can link to e.g.
/// `api/agents.md#dial`. (mdBook's own heading slugs are derived from the whole
/// signature, which is long and brittle.)
fn anchor_id(sig: &str) -> &str {
    let after_dot = sig.rsplit_once('.').map_or(sig, |(_, r)| r);
    after_dot.split('(').next().unwrap_or(after_dot).trim()
}

/// Render one API entry: a stable name anchor, the signature as a plain-text
/// heading, then a meta line linking the receiver / accepted (`Takes`) / return
/// types to their pages, the description and examples. Each overload is its own
/// entry (one signature each).
fn render_entry(md: &mut String, level: &str, e: &Entry) {
    let sig = e.sigs.first().map_or("", String::as_str);
    // A clean, stable anchor by name (the heading's own slug is signature-derived).
    md.push_str(&format!("<a id=\"{}\"></a>\n\n", anchor_id(sig)));
    md.push_str(&format!("{level} {sig}\n\n"));
    // Links live here, not in the heading: a linked type inside an mdBook heading
    // truncates its on-page TOC entry.
    let mut meta: Vec<String> = Vec::new();
    if let Some(r) = &e.receiver {
        meta.push(format!("**Receiver** {}", type_md(r)));
    }
    let takes = sig_types(sig);
    if !takes.is_empty() {
        let links: Vec<String> = takes.iter().map(|t| type_md(t)).collect();
        meta.push(format!("**Takes** {}", links.join(", ")));
    }
    // Prefer a documented `# Returns:` type (e.g. `string?`); else the Rhai metadata
    // type, dropping a bare `any` (a `?`/dynamic return is noise — the description
    // already says what comes back).
    let ret = e.doc_returns.clone().or_else(|| {
        e.returns
            .as_deref()
            .map(ret_display)
            .filter(|d| *d != "any")
            .map(str::to_string)
    });
    if let Some(ret) = ret {
        meta.push(format!("**Returns** {}", type_md(&ret)));
    }
    if !meta.is_empty() {
        md.push_str(&meta.join(" · "));
        md.push_str("\n\n");
    }
    for s in &e.summaries {
        md.push_str(s);
        md.push_str("\n\n");
    }
    for ex in &e.examples {
        md.push_str("**Example**\n\n```rust\n");
        md.push_str(ex);
        md.push_str("\n```\n\n");
    }
}

/// The API split into one Markdown page per section: `(slug, title, body)` in
/// section order. Each `body` is an `# <Title>` page with its entries — the source
/// for the mdBook chapters (one chapter per section, for a navigable sidebar).
///
/// Type sections (those with a constructor) group their entries under `Constructor`
/// / `Members` / `Helpers` headings so the constructor stands apart from the
/// methods; other sections list their entries flat.
fn book_sections(engine: &Engine) -> Result<Vec<(String, &'static str, String)>> {
    let entries = api_entries(engine)?;
    // Group entries per section, preserving the (rank, name) order from the key.
    // `(rank, display-name, entry)` per item.
    type Item<'a> = (u8, &'a String, &'a Entry);
    let mut sections: Vec<(&'static str, Vec<Item>)> = Vec::new();
    for ((sec, rank, display), e) in &entries {
        let title = sec.1;
        if sections.last().map(|(t, _)| *t) != Some(title) {
            sections.push((title, Vec::new()));
        }
        sections.last_mut().unwrap().1.push((*rank, display, e));
    }

    let mut pages = Vec::new();
    for (title, items) in sections {
        let mut body = format!("# {title}\n\n");
        // Group into Constructor / Methods / Fields / Helpers only when the section
        // mixes kinds; a single-kind section (e.g. Peer = only fields) stays a flat
        // list. Items are rank-sorted, so first != last rank ⇒ more than one kind.
        let mixed = items.first().map(|(r, _, _)| r) != items.last().map(|(r, _, _)| r);
        if mixed {
            for (rank, label) in [
                (0u8, "Constructor"),
                (1, "Methods"),
                (2, "Fields"),
                (3, "Helpers"),
            ] {
                let group: Vec<_> = items.iter().filter(|(r, _, _)| *r == rank).collect();
                if group.is_empty() {
                    continue;
                }
                body.push_str(&format!("## {label}\n\n"));
                for (_, _, e) in group {
                    render_entry(&mut body, "###", e);
                }
            }
        } else {
            for (_, _, e) in &items {
                render_entry(&mut body, "##", e);
            }
        }
        pages.push((slug(title), title, body));
    }
    pages.push(call_state_section());
    Ok(pages)
}

/// The `State::*` constants `agent.state` is compared against, with descriptions.
/// (`CallState` variants aren't functions, so they don't appear in the metadata;
/// this is the one place they're documented.)
const CALL_STATES: &[(&str, &str)] = &[
    ("Idle", "No active call."),
    (
        "Ringing",
        "A call is ringing — incoming or outgoing — but not yet answered.",
    ),
    ("Established", "The call is connected and media is flowing."),
];

/// The hand-rolled "Call state" section: the `CallState` type returned by
/// `agent.state`, the `State::*` constants and how to compare them. Returned by
/// [`book_sections`] so it's written and snapshot-tested like the rest.
fn call_state_section() -> (String, &'static str, String) {
    let mut body = String::from(
        "# Call state\n\n\
         `agent.state` returns a **`CallState`** — a call's current phase. Compare it \
         against the `State::*` constants, usually inside `await_until`:\n\n\
         ```rust\n\
         await_until(|| assert(callee.state).equals(State::Ringing));\n\
         ```\n\n",
    );
    for (name, desc) in CALL_STATES {
        body.push_str(&format!("- `State::{name}` — {desc}\n"));
    }
    body.push('\n');
    ("call-state".to_string(), "Call state", body)
}

/// Write the scenario API as one Markdown page per section into `dir` (the mdBook
/// `src/api` directory), named `<slug>.md`. These are the generated API chapters.
/// Sub-type sections nested under a parent in the overview (parent → children).
const API_NESTING: &[(&str, &[&str])] = &[
    ("Agents", &["Peer", "Call state", "AudioSpec"]),
    ("HTTP mock server", &["Mock request"]),
];

/// One-line blurb per section for the overview (`index.md`). Empty for an unlisted
/// section (the link still renders); the snapshot test flags drift either way.
fn section_blurb(title: &str) -> &'static str {
    match title {
        "Scenario structure" => {
            "defining and isolating tests: `scenario`, `setup`, `teardown`, `skip`."
        }
        "Flow and timing" => "`await_until`, `wait`, `parallel`, `default_timeout`.",
        "Agents" => {
            "create SIP endpoints and drive calls: register, dial, accept, transfer, DTMF, audio."
        }
        "Peer" => "the remote party of the active call.",
        "Call state" => "the `State::*` phases for `agent.state`.",
        "AudioSpec" => "audio sources for `send_audio` (`tone`, `file`, `silent`).",
        "Assertions and matchers" => {
            "the fluent `assert(x).<matcher>(…)`, used inside `await_until`."
        }
        "HTTP" => "`http(…)` requests and the response.",
        "HTTP mock server" => "`mock_server(…)`, routes and responders for webhook-driven flows.",
        "Mock request" => "the recorded request a responder/assertion sees.",
        "Environment" => "`env`, `load_env` — credentials stay out of scripts.",
        "Utilities" => "`log`, `uuid`.",
        _ => "",
    }
}

/// The generated API overview (`index.md`): every section linked, sub-types nested,
/// each with a one-line blurb — the landing page for the API reference.
fn api_index_body(engine: &Engine) -> Result<String> {
    use std::collections::{HashMap, HashSet};
    let sections = book_sections(engine)?;
    let slug_of: HashMap<&str, &str> = sections.iter().map(|(s, t, _)| (*t, s.as_str())).collect();
    let children: HashSet<&str> = API_NESTING
        .iter()
        .flat_map(|(_, cs)| cs.iter().copied())
        .collect();

    let line = |indent: &str, title: &str, slug: &str| {
        let b = section_blurb(title);
        let suffix = if b.is_empty() {
            String::new()
        } else {
            format!(" — {b}")
        };
        format!("{indent}- [{title}]({slug}.md){suffix}\n")
    };

    let mut out = String::from(
        "# API reference\n\n\
         The complete scenario vocabulary, generated from the engine (so it never \
         drifts from the code) — organized by the thing you're working with:\n\n",
    );
    for (slug, title, _) in &sections {
        if children.contains(title) {
            continue; // listed under its parent
        }
        out.push_str(&line("", title, slug));
        if let Some((_, cs)) = API_NESTING.iter().find(|(p, _)| p == title) {
            for child in *cs {
                if let Some(cslug) = slug_of.get(child) {
                    out.push_str(&line("  ", child, cslug));
                }
            }
        }
    }
    out.push_str(
        "\nNew to it? Start with [Your first scenario](../your-first-scenario.md), \
         then [Writing scenarios](../writing-scenarios.md).\n\n\
         For editors and agents, the whole API is also available as \
         [Rhai type definitions](../ringo-flow.d.rhai) (`.d.rhai`) — point the Rhai \
         language server at it for completion and hover.\n",
    );
    Ok(out)
}

pub fn write_book_api(dir: &Path) -> Result<()> {
    let (_rt, engine) = doc_engine()?;
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    for (slug, _title, body) in book_sections(&engine)? {
        let path = dir.join(format!("{slug}.md"));
        std::fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    }
    let index = dir.join("index.md");
    std::fs::write(&index, api_index_body(&engine)?)
        .with_context(|| format!("write {}", index.display()))?;
    println!("wrote API pages to {}", dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn book_api_pages_are_current() {
        // The committed mdBook API pages are generated; this fails if they drift
        // from the engine's registered API so they can't go stale silently.
        let (_rt, engine) = super::doc_engine().unwrap();
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/src/ringo-flow/api");
        for (slug, _title, body) in super::book_sections(&engine).unwrap() {
            let path = format!("{dir}/{slug}.md");
            let committed = std::fs::read_to_string(&path).unwrap_or_default();
            assert_eq!(
                body, committed,
                "{path} is stale — regenerate with \
                 `cargo run -p ringo-flow -- docs docs/src/ringo-flow/api`"
            );
        }
        let index = format!("{dir}/index.md");
        assert_eq!(
            super::api_index_body(&engine).unwrap(),
            std::fs::read_to_string(&index).unwrap_or_default(),
            "{index} is stale — regenerate with \
             `cargo run -p ringo-flow -- docs docs/src/ringo-flow/api`"
        );
    }

    #[test]
    fn rhai_definitions_are_current() {
        // The committed .d.rhai (served from the docs, for the Rhai LSP and agents)
        // is generated; fail if it drifts from the engine. Output is deterministic.
        let tmp = std::env::temp_dir().join("ringo-flow-defs.d.rhai");
        super::write_definitions(&tmp).unwrap();
        let generated = std::fs::read_to_string(&tmp).unwrap();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/src/ringo-flow/ringo-flow.d.rhai"
        );
        let committed = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            generated, committed,
            "{path} is stale — regenerate with \
             `cargo run -p ringo-flow -- definitions docs/src/ringo-flow/ringo-flow.d.rhai`"
        );
    }
}
