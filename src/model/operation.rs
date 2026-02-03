//! Operation model for jj operation history

/// Represents a jj operation from `jj op log`
#[derive(Debug, Clone)]
pub struct Operation {
    /// Operation ID (e.g., "75ea3c2331bf")
    pub id: String,
    /// User who performed the operation
    pub user: String,
    /// Timestamp (e.g., "2026-02-02 11:25:54 +09:00")
    pub timestamp: String,
    /// Operation description (e.g., "snapshot working copy")
    pub description: String,
    /// Is this the current operation? (first in list)
    pub is_current: bool,
}

impl Operation {
    /// Get short ID for display (first 12 chars)
    pub fn short_id(&self) -> &str {
        &self.id[..12.min(self.id.len())]
    }

    /// Format timestamp for display (extract relative or short form)
    pub fn display_timestamp(&self) -> &str {
        // Return as-is for now, can be enhanced later
        &self.timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_id() {
        let op = Operation {
            id: "75ea3c2331bf1234567890".to_string(),
            user: "user@host".to_string(),
            timestamp: "2026-02-02 11:25:54".to_string(),
            description: "snapshot working copy".to_string(),
            is_current: true,
        };
        assert_eq!(op.short_id(), "75ea3c2331bf");
    }

    #[test]
    fn test_short_id_short_input() {
        let op = Operation {
            id: "abc".to_string(),
            user: "user@host".to_string(),
            timestamp: "2026-02-02 11:25:54".to_string(),
            description: "test".to_string(),
            is_current: false,
        };
        assert_eq!(op.short_id(), "abc");
    }
}
