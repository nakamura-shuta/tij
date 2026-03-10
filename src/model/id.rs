//! Newtype wrappers for jj IDs
//!
//! Provides compile-time distinction between change IDs (UI identifier)
//! and commit IDs (command execution identifier), preventing accidental
//! mix-ups that cause bugs with divergent changes.

use std::fmt;

/// Truncate an ID to 8 characters for display.
fn short_id(id: &str) -> &str {
    &id[..8.min(id.len())]
}

/// jj change ID — UI identifier used for highlighting, search, cache keys, `--change` flag.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ChangeId(String);

/// jj commit ID — command execution identifier used for `-r` flag, `jj show`, comparisons.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CommitId(String);

// ─────────────────────────────────────────────────────────────────────────────
// ChangeId
// ─────────────────────────────────────────────────────────────────────────────

impl ChangeId {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn short(&self) -> &str {
        short_id(&self.0)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn starts_with(&self, prefix: &str) -> bool {
        self.0.starts_with(prefix)
    }

    pub fn contains(&self, s: &str) -> bool {
        self.0.contains(s)
    }

    pub fn chars(&self) -> std::str::Chars<'_> {
        self.0.chars()
    }

    pub fn to_lowercase(&self) -> String {
        self.0.to_lowercase()
    }
}

impl fmt::Display for ChangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for ChangeId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for ChangeId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for ChangeId {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}

impl PartialEq<ChangeId> for String {
    fn eq(&self, other: &ChangeId) -> bool {
        *self == other.0
    }
}

impl From<String> for ChangeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ChangeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CommitId
// ─────────────────────────────────────────────────────────────────────────────

impl CommitId {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn short(&self) -> &str {
        short_id(&self.0)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn starts_with(&self, prefix: &str) -> bool {
        self.0.starts_with(prefix)
    }

    pub fn contains(&self, s: &str) -> bool {
        self.0.contains(s)
    }

    pub fn chars(&self) -> std::str::Chars<'_> {
        self.0.chars()
    }
}

impl fmt::Display for CommitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for CommitId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for CommitId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for CommitId {
    fn eq(&self, other: &String) -> bool {
        self.0 == *other
    }
}

impl PartialEq<CommitId> for String {
    fn eq(&self, other: &CommitId) -> bool {
        *self == other.0
    }
}

impl From<String> for CommitId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CommitId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_id_display() {
        let id = ChangeId::new("abc12345".to_string());
        assert_eq!(format!("{}", id), "abc12345");
    }

    #[test]
    fn test_commit_id_display() {
        let id = CommitId::new("def67890".to_string());
        assert_eq!(format!("{}", id), "def67890");
    }

    #[test]
    fn test_change_id_eq_str() {
        let id = ChangeId::new("abc12345".to_string());
        assert!(id == "abc12345");
        assert!(id != "xyz99999");
    }

    #[test]
    fn test_commit_id_eq_str() {
        let id = CommitId::new("def67890".to_string());
        assert!(id == "def67890");
        assert!(id != "xyz99999");
    }

    #[test]
    fn test_change_id_short() {
        let id = ChangeId::new("abcdef1234567890".to_string());
        assert_eq!(id.short(), "abcdef12");
    }

    #[test]
    fn test_commit_id_short() {
        let id = CommitId::new("abcdef1234567890".to_string());
        assert_eq!(id.short(), "abcdef12");
    }

    #[test]
    fn test_default_is_empty() {
        assert!(ChangeId::default().is_empty());
        assert!(CommitId::default().is_empty());
    }

    #[test]
    fn test_starts_with() {
        let id = ChangeId::new("abc12345".to_string());
        assert!(id.starts_with("abc"));
        assert!(!id.starts_with("xyz"));
    }

    #[test]
    fn test_contains() {
        let id = ChangeId::new("abc12345".to_string());
        assert!(id.contains("c12"));
        assert!(!id.contains("xyz"));
    }

    #[test]
    fn test_from_string() {
        let id: ChangeId = "test".into();
        assert_eq!(id.as_str(), "test");
    }

    #[test]
    fn test_hash_works() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(ChangeId::new("abc".to_string()), 42);
        assert_eq!(map.get(&ChangeId::new("abc".to_string())), Some(&42));
    }
}
