//! Custom-header template values with `$placeholder` substitution.
//!
//! Templates are evaluated per outgoing call so that values like `$uuid` can
//! produce a fresh identifier on every INVITE. A single [`HeaderContext`] is
//! shared across all template renders within one call, so multiple headers
//! that reference `$uuid` in the same INVITE see the same value.

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

    /// True if the template references at least one placeholder and therefore
    /// must be re-rendered per call.
    pub fn is_dynamic(&self) -> bool {
        PLACEHOLDERS.iter().any(|p| self.0.contains(p.token))
    }

    /// Substitute all placeholders using the supplied context.
    pub fn render(&self, ctx: &HeaderContext) -> String {
        let mut out = self.0.clone();
        for p in PLACEHOLDERS {
            if out.contains(p.token) {
                out = out.replace(p.token, &(p.value)(ctx));
            }
        }
        out
    }
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
    token: &'static str,
    value: fn(&HeaderContext) -> String,
}

const PLACEHOLDERS: &[Placeholder] = &[Placeholder {
    token: "$uuid",
    value: |c| c.uuid.clone(),
}];

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
        assert!(HeaderTemplate::new("call-$uuid").is_dynamic());
    }

    #[test]
    fn render_substitutes_uuid() {
        let ctx = HeaderContext {
            uuid: "abc-123".into(),
        };
        let t = HeaderTemplate::new("call-$uuid-end");
        assert_eq!(t.render(&ctx), "call-abc-123-end");
    }

    #[test]
    fn render_substitutes_all_occurrences_with_same_value() {
        let ctx = HeaderContext { uuid: "X".into() };
        assert_eq!(HeaderTemplate::new("$uuid/$uuid").render(&ctx), "X/X");
    }

    #[test]
    fn render_leaves_static_template_unchanged() {
        let ctx = HeaderContext { uuid: "X".into() };
        assert_eq!(HeaderTemplate::new("plain").render(&ctx), "plain");
    }
}
