//! Custom-header template values with `${placeholder}` substitution.
//!
//! Templates are evaluated per outgoing call so that values like `${uuid}` can
//! produce a fresh identifier on every INVITE. A single [`HeaderContext`] is
//! shared across all template renders within one call, so multiple headers
//! that reference `${uuid}` in the same INVITE see the same value.
//!
//! Syntax: `${name}` expands a placeholder; `$$` is a literal `$` (so a literal
//! `${uuid}` is written `$${uuid}`). Unknown placeholders and `$` not followed
//! by `{` or `$` are left verbatim.

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderTemplate(String);

impl HeaderTemplate {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The raw template string, with placeholders unresolved.
    pub fn raw(&self) -> &str {
        &self.0
    }

    /// True if the template references at least one known placeholder and
    /// therefore must be re-rendered per call.
    pub fn is_dynamic(&self) -> bool {
        let mut found = false;
        expand(&self.0, |name| {
            lookup(name).map(|_| {
                found = true;
                String::new()
            })
        });
        found
    }

    /// Substitute all placeholders using the supplied context.
    pub fn render(&self, ctx: &HeaderContext) -> String {
        expand(&self.0, |name| lookup(name).map(|p| (p.value)(ctx)))
    }
}

/// Expand `${name}` placeholders in `s`. `sub(name)` returns `Some(value)` for
/// a recognized placeholder or `None` to leave it verbatim. `$$` is a literal
/// `$`; a `$` not followed by `{` or `$` is also left verbatim.
fn expand(s: &str, mut sub: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(idx) = rest.find('$') {
        out.push_str(&rest[..idx]);
        let after = &rest[idx + 1..];
        if let Some(stripped) = after.strip_prefix('$') {
            out.push('$');
            rest = stripped;
        } else if let Some(stripped) = after.strip_prefix('{') {
            match stripped.find('}') {
                Some(end) => {
                    let name = &stripped[..end];
                    match sub(name) {
                        Some(value) => out.push_str(&value),
                        None => out.push_str(&rest[..idx + 2 + end + 1]),
                    }
                    rest = &stripped[end + 1..];
                }
                None => {
                    // No closing brace: leave the rest verbatim.
                    out.push_str(&rest[idx..]);
                    return out;
                }
            }
        } else {
            out.push('$');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Per-call values shared across all templates rendered for the same INVITE.
pub struct HeaderContext {
    pub uuid: String,
}

impl HeaderContext {
    pub fn for_call() -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
        }
    }
}

struct Placeholder {
    name: &'static str,
    value: fn(&HeaderContext) -> String,
}

const PLACEHOLDERS: &[Placeholder] = &[Placeholder {
    name: "uuid",
    value: |c| c.uuid.clone(),
}];

fn lookup(name: &str) -> Option<&'static Placeholder> {
    PLACEHOLDERS.iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_template_is_not_dynamic() {
        assert!(!HeaderTemplate::new("plain").is_dynamic());
        assert!(!HeaderTemplate::new("with$dollar but no token").is_dynamic());
    }

    #[test]
    fn uuid_placeholder_is_dynamic() {
        assert!(HeaderTemplate::new("call-${uuid}").is_dynamic());
    }

    #[test]
    fn unknown_placeholder_is_not_dynamic() {
        assert!(!HeaderTemplate::new("call-${unknown}").is_dynamic());
    }

    #[test]
    fn bare_token_without_braces_is_not_dynamic() {
        assert!(!HeaderTemplate::new("call-$uuid").is_dynamic());
    }

    #[test]
    fn escaped_placeholder_is_not_dynamic() {
        assert!(!HeaderTemplate::new("$${uuid}").is_dynamic());
    }

    #[test]
    fn render_substitutes_uuid() {
        let ctx = HeaderContext {
            uuid: "abc-123".into(),
        };
        let t = HeaderTemplate::new("call-${uuid}-end");
        assert_eq!(t.render(&ctx), "call-abc-123-end");
    }

    #[test]
    fn render_substitutes_all_occurrences_with_same_value() {
        let ctx = HeaderContext { uuid: "X".into() };
        assert_eq!(HeaderTemplate::new("${uuid}/${uuid}").render(&ctx), "X/X");
    }

    #[test]
    fn render_leaves_static_template_unchanged() {
        let ctx = HeaderContext { uuid: "X".into() };
        assert_eq!(HeaderTemplate::new("plain").render(&ctx), "plain");
    }

    #[test]
    fn render_does_not_match_token_prefix() {
        let ctx = HeaderContext { uuid: "X".into() };
        assert_eq!(HeaderTemplate::new("${uuid2}").render(&ctx), "${uuid2}");
    }

    #[test]
    fn render_unescapes_double_dollar() {
        let ctx = HeaderContext { uuid: "X".into() };
        assert_eq!(HeaderTemplate::new("$${uuid}").render(&ctx), "${uuid}");
        assert_eq!(HeaderTemplate::new("a$$b").render(&ctx), "a$b");
    }

    #[test]
    fn render_leaves_lone_dollar_and_unclosed_brace_verbatim() {
        let ctx = HeaderContext { uuid: "X".into() };
        assert_eq!(
            HeaderTemplate::new("5$ and ${uuid").render(&ctx),
            "5$ and ${uuid"
        );
    }
}
