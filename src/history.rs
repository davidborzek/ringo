use std::{collections::VecDeque, fs, path::PathBuf};

pub fn path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/ringo/history")
}

pub fn load() -> VecDeque<String> {
    let p = path();
    if let Ok(content) = fs::read_to_string(&p) {
        content
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect()
    } else {
        VecDeque::new()
    }
}

fn save(history: &VecDeque<String>) {
    let p = path();
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = history.iter().cloned().collect::<Vec<_>>().join("\n");
    if let Err(e) = fs::write(&p, &content) {
        crate::rlog!(Warn, "dial history write failed: {}", e);
    }
}

/// Prepend `entry` (deduplicating), cap at 1000, persist to disk.
pub fn push(history: &mut VecDeque<String>, entry: String) {
    history.retain(|e| e != &entry);
    history.push_front(entry);
    if history.len() > 1000 {
        history.pop_back();
    }
    save(history);
}

/// Subsequence fuzzy filter — keeps order (newest first).
pub fn fuzzy_filter<'a>(history: &'a VecDeque<String>, query: &str) -> Vec<&'a str> {
    history
        .iter()
        .filter(|e| fuzzy_match(query, e))
        .map(|e| e.as_str())
        .collect()
}

fn fuzzy_match(query: &str, text: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let text_chars: Vec<char> = text.to_lowercase().chars().collect();
    let mut t = 0;
    for qc in query.to_lowercase().chars() {
        match text_chars[t..].iter().position(|&c| c == qc) {
            Some(pos) => t += pos + 1,
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deque(items: &[&str]) -> VecDeque<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    // ── fuzzy_filter ───────────────────────────────────────────────────────────

    #[test]
    fn empty_query_returns_all() {
        let h = deque(&["sip:alice@example.com", "sip:bob@example.com"]);
        assert_eq!(fuzzy_filter(&h, "").len(), 2);
    }

    #[test]
    fn exact_substring_matches() {
        let h = deque(&["sip:alice@example.com", "sip:bob@example.com"]);
        let r = fuzzy_filter(&h, "alice");
        assert_eq!(r, vec!["sip:alice@example.com"]);
    }

    #[test]
    fn subsequence_matches() {
        let h = deque(&["sip:alice@example.com"]);
        assert_eq!(fuzzy_filter(&h, "alx").len(), 1); // a-l-x all present in order
        assert_eq!(fuzzy_filter(&h, "xyz").len(), 0);
    }

    #[test]
    fn case_insensitive_match() {
        let h = deque(&["sip:Alice@Example.com"]);
        assert_eq!(fuzzy_filter(&h, "alice").len(), 1);
    }

    #[test]
    fn no_match_returns_empty() {
        let h = deque(&["sip:alice@example.com"]);
        assert_eq!(fuzzy_filter(&h, "zzz").len(), 0);
    }

    // ── push ───────────────────────────────────────────────────────────────────

    #[test]
    fn push_prepends() {
        let mut h = deque(&["b", "c"]);
        h.retain(|e| e != "a");
        h.push_front("a".to_string());
        assert_eq!(h[0], "a");
    }

    #[test]
    fn push_deduplicates() {
        let mut h = deque(&["a", "b", "c"]);
        h.retain(|e| e != "b");
        h.push_front("b".to_string());
        assert_eq!(h.iter().filter(|e| *e == "b").count(), 1);
        assert_eq!(h[0], "b");
    }
}
