use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub numbers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContactsFile {
    #[serde(default)]
    contacts: Vec<Contact>,
}

pub fn contacts_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("ringo")
            .join("contacts.toml"),
    )
}

pub fn load() -> Vec<Contact> {
    let Some(path) = contacts_path() else {
        return Vec::new();
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => match toml::from_str::<ContactsFile>(&content) {
            Ok(file) => file.contacts,
            Err(e) => {
                crate::rlog!(Warn, "contacts parse error: {}", e);
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    }
}

pub fn save(contacts: &[Contact]) {
    let Some(path) = contacts_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = ContactsFile {
        contacts: contacts.to_vec(),
    };
    match toml::to_string_pretty(&file) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&path, content) {
                crate::rlog!(Warn, "contacts write failed: {}", e);
            }
        }
        Err(e) => {
            crate::rlog!(Warn, "contacts serialize failed: {}", e);
        }
    }
}

/// Extract the user part from a SIP URI: `sip:user@domain` → `user`.
/// Passes through as-is if not a SIP URI.
pub fn extract_user_part(uri: &str) -> &str {
    let s = uri.strip_prefix("sip:").unwrap_or(uri);
    match s.find('@') {
        Some(pos) => &s[..pos],
        None => s,
    }
}

/// Strip non-digit characters, then trim leading zeros.
/// `+49555xxx` → `49555xxx`, `0555xxx` → `555xxx`, `0049555xxx` → `49555xxx`
fn normalize_digits(s: &str) -> String {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.trim_start_matches('0').to_string()
}

/// Check if two phone numbers refer to the same destination.
/// Exact string match first, then suffix match on normalized digits (min 7 digits).
pub fn numbers_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let da = normalize_digits(a);
    let db = normalize_digits(b);
    if da.is_empty() || db.is_empty() {
        return false;
    }
    if da == db {
        return true;
    }
    let (short, long) = if da.len() <= db.len() {
        (&da, &db)
    } else {
        (&db, &da)
    };
    short.len() >= 7 && long.ends_with(short.as_str())
}

/// Resolve a display name by matching the user part of a URI against contact numbers.
pub fn resolve_name<'a>(contacts: &'a [Contact], uri: &str) -> Option<&'a str> {
    let user = extract_user_part(uri);
    contacts
        .iter()
        .find(|c| c.numbers.iter().any(|n| numbers_match(n, user)))
        .map(|c| c.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_user_part_sip_uri() {
        assert_eq!(extract_user_part("sip:+49123@example.com"), "+49123");
    }

    #[test]
    fn extract_user_part_no_prefix() {
        assert_eq!(extract_user_part("+49123@example.com"), "+49123");
    }

    #[test]
    fn extract_user_part_no_at() {
        assert_eq!(extract_user_part("sip:+49123"), "+49123");
    }

    #[test]
    fn extract_user_part_plain_number() {
        assert_eq!(extract_user_part("+49123456789"), "+49123456789");
    }

    #[test]
    fn resolve_name_found() {
        let contacts = vec![Contact {
            name: "Alice".into(),
            numbers: vec!["+49123".into()],
        }];
        assert_eq!(
            resolve_name(&contacts, "sip:+49123@example.com"),
            Some("Alice")
        );
    }

    #[test]
    fn resolve_name_not_found() {
        let contacts: Vec<Contact> = vec![];
        assert_eq!(resolve_name(&contacts, "sip:+49999@example.com"), None);
    }

    #[test]
    fn resolve_name_plain_number() {
        let contacts = vec![Contact {
            name: "Bob".into(),
            numbers: vec!["+49123".into()],
        }];
        assert_eq!(resolve_name(&contacts, "+49123"), Some("Bob"));
    }

    #[test]
    fn numbers_match_exact() {
        assert!(numbers_match("+49155512345", "+49155512345"));
    }

    #[test]
    fn numbers_match_local_vs_international() {
        // 01555... (local) vs +491555... (international)
        assert!(numbers_match("015551234567", "+4915551234567"));
    }

    #[test]
    fn numbers_match_local_vs_no_plus() {
        assert!(numbers_match("015551234567", "4915551234567"));
    }

    #[test]
    fn numbers_match_international_saved_local_calling() {
        // Contact saved as +491555..., call comes from 01555...
        assert!(numbers_match("+4915551234567", "015551234567"));
    }

    #[test]
    fn numbers_match_no_plus_saved_local_calling() {
        // Contact saved as 491555..., call comes from 01555...
        assert!(numbers_match("4915551234567", "015551234567"));
    }

    #[test]
    fn numbers_match_double_zero_prefix() {
        assert!(numbers_match("004915551234567", "+4915551234567"));
    }

    #[test]
    fn numbers_match_short_number_no_false_positive() {
        // Too few digits for suffix match
        assert!(!numbers_match("123", "4915551234"));
    }

    #[test]
    fn numbers_match_alphanumeric_exact() {
        assert!(numbers_match("alice.work", "alice.work"));
    }

    #[test]
    fn numbers_match_alphanumeric_no_false_match() {
        assert!(!numbers_match("alice.work", "bob.work"));
    }

    #[test]
    fn resolve_name_local_vs_international() {
        let contacts = vec![Contact {
            name: "Alice".into(),
            numbers: vec!["015551234567".into()],
        }];
        assert_eq!(
            resolve_name(&contacts, "sip:+4915551234567@example.com"),
            Some("Alice")
        );
    }

    #[test]
    fn resolve_name_multiple_numbers() {
        let contacts = vec![Contact {
            name: "Alice".into(),
            numbers: vec!["+49123".into(), "alice.work".into()],
        }];
        assert_eq!(
            resolve_name(&contacts, "sip:alice.work@example.com"),
            Some("Alice")
        );
    }
}
