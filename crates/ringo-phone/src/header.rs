//! Inbound-header views for incoming calls.
//!
//! The custom-header templates ([`HeaderTemplate`], [`HeaderContext`]) live in
//! `ringo_profile::header`; this module re-exports them so existing
//! `ringo::header::…` paths keep working.

pub use ringo_profile::header::{HeaderContext, HeaderTemplate};

/// Render an inbound-header view template against a call's received headers.
///
/// `${Header-Name}` is substituted with the header's value (case-insensitive
/// match); a referenced-but-absent header expands to empty. Returns `None` when
/// the template references placeholders but none of them are present on this
/// call, so irrelevant views are skipped rather than shown blank. A template
/// with no placeholders (static text) always renders.
pub(crate) fn render_header_view(template: &str, headers: &[(String, String)]) -> Option<String> {
    let mut saw_placeholder = false;
    let mut matched = false;
    let rendered = ringo_profile::header::expand(template, |name| {
        saw_placeholder = true;
        match headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
            Some((_, v)) => {
                matched = true;
                Some(v.clone())
            }
            None => Some(String::new()),
        }
    });
    if saw_placeholder && !matched {
        None
    } else {
        Some(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_view_substitutes_case_insensitively() {
        let h = vec![("X-Call-Sid".to_string(), "abc123".to_string())];
        assert_eq!(
            render_header_view("http://dbg/${x-call-sid}", &h).as_deref(),
            Some("http://dbg/abc123")
        );
    }

    #[test]
    fn header_view_skips_when_referenced_header_absent() {
        let h = vec![("X-Other".to_string(), "1".to_string())];
        assert_eq!(render_header_view("id=${X-Call-Sid}", &h), None);
    }

    #[test]
    fn header_view_static_text_always_renders() {
        assert_eq!(
            render_header_view("just text", &[]).as_deref(),
            Some("just text")
        );
    }

    #[test]
    fn header_view_absent_placeholder_expands_empty_when_another_matches() {
        let h = vec![("A".to_string(), "1".to_string())];
        assert_eq!(render_header_view("${A}/${B}", &h).as_deref(), Some("1/"));
    }
}
