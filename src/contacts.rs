use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub numbers: Vec<String>,
}

#[derive(Debug, Deserialize)]
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

/// Extract the user part from a SIP URI: `sip:user@domain` → `user`.
/// Passes through as-is if not a SIP URI.
pub fn extract_user_part(uri: &str) -> &str {
    let s = uri.strip_prefix("sip:").unwrap_or(uri);
    match s.find('@') {
        Some(pos) => &s[..pos],
        None => s,
    }
}

/// Resolve a display name by matching the user part of a URI against contact numbers.
pub fn resolve_name<'a>(contacts: &'a [Contact], uri: &str) -> Option<&'a str> {
    let user = extract_user_part(uri);
    contacts
        .iter()
        .find(|c| c.numbers.iter().any(|n| n == user))
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
