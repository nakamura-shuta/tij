//! Command history model for tracking executed jj commands

use std::collections::VecDeque;
use std::time::SystemTime;

/// Maximum number of command records to keep.
///
/// 500 (was 200): the history now records read invocations too (command
/// transparency P1), so writes need more headroom before eviction.
const DEFAULT_CAPACITY: usize = 500;

/// Status of a command execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandStatus {
    Success,
    Failed,
}

/// What kind of jj invocation a record represents (command transparency P1).
///
/// `Read` = observes state only (log/show/status/… and `--dry-run` prechecks);
/// `Write` = mutates the repo; `Interactive` = spawned with inherited stdio
/// (editor/diff-editor sessions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Read,
    Write,
    Interactive,
}

impl CommandKind {
    /// Short list tag: `[R]` / `[W]` / `[I]`
    pub fn tag(&self) -> &'static str {
        match self {
            CommandKind::Read => "R",
            CommandKind::Write => "W",
            CommandKind::Interactive => "I",
        }
    }
}

/// A single command execution record
#[derive(Debug, Clone)]
pub struct CommandRecord {
    /// Human-readable operation name (e.g. "Describe", "Push")
    pub operation: String,
    /// The full argv actually passed to jj (including `--color=never`,
    /// `-R <path>`, `--no-integrate-operation` — the honest command line)
    pub args: Vec<String>,
    /// Read / Write / Interactive
    pub kind: CommandKind,
    /// How many consecutive identical executions this record stands for
    /// (reads collapse — see [`CommandHistory::push_collapsing`])
    pub repeat: u32,
    /// When the command was executed
    pub timestamp: SystemTime,
    /// How long the command took in milliseconds
    pub duration_ms: u128,
    /// Whether the command succeeded or failed
    pub status: CommandStatus,
    /// Error text if the command failed — jj's **full** stderr, which is
    /// routinely multi-line (`Error: …` then `Hint: …`). The list row shows
    /// none of it; the Enter detail renders it line by line.
    pub error: Option<String>,
}

impl CommandRecord {
    /// The record as a shell-pasteable command line: `jj` + each arg quoted
    /// for POSIX shells. Pasting this into a terminal re-runs exactly what
    /// tij executed (the learning loop behind the `y` key).
    pub fn shell_command_line(&self) -> String {
        let mut out = String::from("jj");
        for arg in &self.args {
            out.push(' ');
            out.push_str(&shell_quote(arg));
        }
        out
    }
}

/// Quote a single argument for POSIX shells. Plain identifiers pass through;
/// anything else is single-quoted (with embedded `'` as `'\''`).
pub fn shell_quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./:@=+,".contains(c));
    if plain {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// Render args for a one-line listing: the value following `-T` (jj display
/// templates run hundreds of chars) is elided to `<…>`. Detail views show the
/// full argv; this is for list rows only.
pub fn display_args(args: &[String]) -> String {
    let mut out: Vec<&str> = Vec::with_capacity(args.len());
    let mut elide_next = false;
    for arg in args {
        if elide_next {
            out.push("<…>");
            elide_next = false;
        } else {
            if arg == "-T" || arg == "--template" {
                elide_next = true;
            }
            out.push(arg);
        }
    }
    out.join(" ")
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
    /// Create a new empty history with default capacity (500)
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

    /// Push, collapsing consecutive identical **reads** into one row.
    ///
    /// A read whose argv and status match the newest record bumps its
    /// `repeat` (and refreshes timestamp/duration) instead of appending —
    /// preview navigation re-runs the same `jj show` dozens of times and
    /// would otherwise drown the history. Writes are never collapsed: each
    /// one is a distinct operation worth its own row.
    pub fn push_collapsing(&mut self, record: CommandRecord) {
        if record.kind == CommandKind::Read
            && let Some(last) = self.records.back_mut()
            && last.kind == CommandKind::Read
            && last.args == record.args
            && last.status == record.status
        {
            last.repeat = last.repeat.saturating_add(1);
            last.timestamp = record.timestamp;
            last.duration_ms = record.duration_ms;
            return;
        }
        self.push(record);
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
            kind: CommandKind::Write,
            repeat: 1,
            timestamp: SystemTime::now(),
            duration_ms: 42,
            status,
            error: None,
        }
    }

    fn make_read(args: &[&str]) -> CommandRecord {
        CommandRecord {
            operation: "log (read)".to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            kind: CommandKind::Read,
            repeat: 1,
            timestamp: SystemTime::now(),
            duration_ms: 10,
            status: CommandStatus::Success,
            error: None,
        }
    }

    #[test]
    fn push_collapsing_merges_identical_reads() {
        let mut history = CommandHistory::new();
        history.push_collapsing(make_read(&["log", "-r", "all()"]));
        history.push_collapsing(make_read(&["log", "-r", "all()"]));
        history.push_collapsing(make_read(&["log", "-r", "all()"]));
        assert_eq!(history.len(), 1);
        assert_eq!(history.records()[0].repeat, 3);

        // Different argv → new row
        history.push_collapsing(make_read(&["show", "-r", "abc"]));
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn push_collapsing_never_merges_writes() {
        let mut history = CommandHistory::new();
        let w = || make_record("New change", CommandStatus::Success);
        history.push_collapsing(w());
        history.push_collapsing(w());
        assert_eq!(history.len(), 2, "writes get one row each");
    }

    #[test]
    fn push_collapsing_read_after_write_is_new_row() {
        let mut history = CommandHistory::new();
        history.push_collapsing(make_record("New change", CommandStatus::Success));
        history.push_collapsing(make_read(&["log"]));
        history.push_collapsing(make_read(&["log"]));
        assert_eq!(history.len(), 2);
        assert_eq!(history.records()[1].repeat, 2);
    }

    #[test]
    fn shell_command_line_is_pasteable() {
        let mut rec = make_read(&[
            "--color=never",
            "log",
            "-r",
            "all()",
            "-T",
            r#"separate("\t", commit_id.short())"#,
        ]);
        rec.args.insert(0, "-R".to_string());
        rec.args.insert(1, "/tmp/my repo".to_string());
        let line = rec.shell_command_line();
        assert_eq!(
            line,
            r#"jj -R '/tmp/my repo' --color=never log -r 'all()' -T 'separate("\t", commit_id.short())'"#
        );
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("plain-arg_1.0"), "plain-arg_1.0");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote(""), "''");
    }

    #[test]
    fn display_args_elides_template_value_only() {
        let args: Vec<String> = [
            "--color=never",
            "log",
            "-T",
            "separate(...300 chars...)",
            "--limit",
            "200",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(display_args(&args), "--color=never log -T <…> --limit 200");
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
