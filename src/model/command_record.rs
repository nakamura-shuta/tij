//! Command history model for tracking executed jj commands

use std::collections::VecDeque;
use std::time::SystemTime;

/// Maximum number of command records to keep
const DEFAULT_CAPACITY: usize = 200;

/// Status of a command execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandStatus {
    Success,
    Failed,
}

/// A single command execution record
#[derive(Debug, Clone)]
pub struct CommandRecord {
    /// Human-readable operation name (e.g. "Describe", "Push")
    pub operation: String,
    /// Command arguments (jj subcommand and args, excluding --color=never and --repository)
    pub args: Vec<String>,
    /// When the command was executed
    pub timestamp: SystemTime,
    /// How long the command took in milliseconds
    pub duration_ms: u128,
    /// Whether the command succeeded or failed
    pub status: CommandStatus,
    /// Error message if the command failed
    pub error: Option<String>,
}

/// FIFO history of command executions with bounded capacity
#[derive(Debug)]
pub struct CommandHistory {
    records: VecDeque<CommandRecord>,
    capacity: usize,
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHistory {
    /// Create a new empty history with default capacity (200)
    pub fn new() -> Self {
        Self {
            records: VecDeque::new(),
            capacity: DEFAULT_CAPACITY,
        }
    }

    /// Push a new record, evicting the oldest if at capacity
    pub fn push(&mut self, record: CommandRecord) {
        if self.records.len() >= self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }

    /// Get all records (oldest first)
    pub fn records(&self) -> &VecDeque<CommandRecord> {
        &self.records
    }

    /// Number of records
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the history is empty
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(operation: &str, status: CommandStatus) -> CommandRecord {
        CommandRecord {
            operation: operation.to_string(),
            args: vec!["test".to_string()],
            timestamp: SystemTime::now(),
            duration_ms: 42,
            status,
            error: None,
        }
    }

    #[test]
    fn test_push_and_capacity() {
        let mut history = CommandHistory {
            records: VecDeque::new(),
            capacity: 3,
        };

        for i in 0..5 {
            history.push(make_record(&format!("op{}", i), CommandStatus::Success));
        }

        // Should keep only the last 3
        assert_eq!(history.len(), 3);
        assert_eq!(history.records()[0].operation, "op2");
        assert_eq!(history.records()[1].operation, "op3");
        assert_eq!(history.records()[2].operation, "op4");
    }

    #[test]
    fn test_records_order() {
        let mut history = CommandHistory::new();

        history.push(make_record("first", CommandStatus::Success));
        history.push(make_record("second", CommandStatus::Failed));

        // Newest record is at the end (back)
        assert_eq!(history.records().len(), 2);
        assert_eq!(history.records()[0].operation, "first");
        assert_eq!(history.records()[1].operation, "second");
    }

    #[test]
    fn test_empty_history() {
        let history = CommandHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
    }
}
