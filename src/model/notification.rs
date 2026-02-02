//! Notification model
//!
//! Used for displaying temporary feedback messages (undo/redo results, etc.)

use std::time::Instant;

/// Kind of notification (determines color)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    /// Success - operation completed (green)
    Success,
    /// Info - informational message (cyan)
    Info,
    /// Warning - caution message (yellow)
    Warning,
}

/// A notification to display to the user
#[derive(Debug, Clone)]
pub struct Notification {
    /// The message to display
    pub message: String,
    /// Kind of notification
    pub kind: NotificationKind,
    /// When the notification was created
    pub created_at: Instant,
}

impl Notification {
    /// Create a new notification
    pub fn new(message: impl Into<String>, kind: NotificationKind) -> Self {
        Self {
            message: message.into(),
            kind,
            created_at: Instant::now(),
        }
    }

    /// Create a success notification
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, NotificationKind::Success)
    }

    /// Create an info notification
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message, NotificationKind::Info)
    }

    /// Create a warning notification
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message, NotificationKind::Warning)
    }

    /// Check if the notification has expired (default: 5 seconds)
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() >= 5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_new() {
        let n = Notification::new("Test message", NotificationKind::Success);
        assert_eq!(n.message, "Test message");
        assert_eq!(n.kind, NotificationKind::Success);
    }

    #[test]
    fn test_notification_success() {
        let n = Notification::success("Operation completed");
        assert_eq!(n.kind, NotificationKind::Success);
    }

    #[test]
    fn test_notification_info() {
        let n = Notification::info("FYI");
        assert_eq!(n.kind, NotificationKind::Info);
    }

    #[test]
    fn test_notification_warning() {
        let n = Notification::warning("Careful!");
        assert_eq!(n.kind, NotificationKind::Warning);
    }

    #[test]
    fn test_notification_not_expired_immediately() {
        let n = Notification::success("Test");
        assert!(!n.is_expired());
    }

    #[test]
    fn test_notification_string_conversion() {
        let n = Notification::success(String::from("Owned string"));
        assert_eq!(n.message, "Owned string");
    }
}
