//! jj-specific constants
//!
//! Centralized definitions for jj command names, flags, and special values.

/// jj command binary name
pub const JJ_COMMAND: &str = "jj";

/// jj subcommands
pub mod commands {
    pub const LOG: &str = "log";
    pub const STATUS: &str = "status";
    pub const SHOW: &str = "show";
    pub const DESCRIBE: &str = "describe";
    pub const NEW: &str = "new";
    pub const EDIT: &str = "edit";
    pub const COMMIT: &str = "commit";
    pub const UNDO: &str = "undo";
    pub const SQUASH: &str = "squash";
    pub const ABANDON: &str = "abandon";
    pub const SPLIT: &str = "split";
    pub const OP: &str = "op";
    pub const OP_LOG: &str = "log";
    pub const OP_RESTORE: &str = "restore";
    pub const BOOKMARK: &str = "bookmark";
    pub const BOOKMARK_CREATE: &str = "create";
    pub const BOOKMARK_SET: &str = "set";
    pub const BOOKMARK_DELETE: &str = "delete";
    pub const BOOKMARK_LIST: &str = "list";
    pub const BOOKMARK_TRACK: &str = "track";
    pub const BOOKMARK_UNTRACK: &str = "untrack";
    pub const BOOKMARK_RENAME: &str = "rename";
    pub const BOOKMARK_FORGET: &str = "forget";
    pub const NEXT: &str = "next";
    pub const PREV: &str = "prev";
    pub const REBASE: &str = "rebase";
    pub const ABSORB: &str = "absorb";
    pub const FILE: &str = "file";
    pub const FILE_ANNOTATE: &str = "annotate";
    pub const RESOLVE: &str = "resolve";
    pub const GIT: &str = "git";
    pub const GIT_FETCH: &str = "fetch";
    pub const GIT_PUSH: &str = "push";
    pub const DIFF: &str = "diff";
    pub const GIT_REMOTE: &str = "remote";
    pub const GIT_REMOTE_LIST: &str = "list";
}

/// jj resolve flags
pub mod resolve_flags {
    pub const LIST: &str = "--list";
    pub const TOOL: &str = "--tool";
}

/// jj command flags
pub mod flags {
    /// Disable color output for parsing (global flag, safe for all commands)
    pub const NO_COLOR: &str = "--color=never";
    /// Disable graph output for parsing (jj log only, NOT a global flag)
    pub const NO_GRAPH: &str = "--no-graph";
    /// Specify template
    pub const TEMPLATE: &str = "-T";
    /// Specify revision/revset
    pub const REVISION: &str = "-r";
    /// Specify repository path
    pub const REPO_PATH: &str = "-R";
    /// Specify bookmark for push
    pub const BOOKMARK_FLAG: &str = "--bookmark";
    /// Allow pushing new bookmarks (deprecated in jj 0.37+, but functional)
    pub const ALLOW_NEW: &str = "--allow-new";
    /// List all remotes for bookmark list (jj 0.37+)
    pub const ALL_REMOTES: &str = "--all-remotes";
    /// Named push for new bookmarks (jj 0.37+): --named <bookmark>=<revision>
    pub const NAMED: &str = "--named";
    /// Rebase source (with descendants)
    pub const SOURCE: &str = "-s";
    /// Insert after target revision
    pub const INSERT_AFTER: &str = "-A";
    /// Insert before target revision
    pub const INSERT_BEFORE: &str = "-B";
    /// Dry-run mode (preview only, no actual push)
    pub const DRY_RUN: &str = "--dry-run";
    /// Diff from revision
    pub const FROM: &str = "--from";
    /// Diff to revision
    pub const TO: &str = "--to";
    /// Open editor for interactive editing (e.g., jj describe --edit)
    pub const EDIT_FLAG: &str = "--edit";
    /// Limit number of results
    pub const LIMIT: &str = "--limit";
    /// Reversed display order (oldest first)
    pub const REVERSED: &str = "--reversed";
    /// Push by change ID (creates automatic bookmark)
    pub const CHANGE: &str = "--change";
    /// Specify remote for push/fetch
    pub const REMOTE: &str = "--remote";
}

/// Default limit for log output (no revset)
pub const DEFAULT_LOG_LIMIT: &str = "200";

/// Special jj values
pub mod special {
    /// The root change ID (all 'z' characters)
    ///
    /// In jj, the root commit has a special change ID consisting of all 'z'.
    /// This is used to identify and specially render the root in Log View.
    pub const ROOT_CHANGE_ID: &str = "zzzzzzzz";
}

/// Error detection patterns in jj output
pub mod errors {
    /// Pattern indicating not a jj repository
    pub const NOT_A_REPO: &str = "There is no jj repo";
}

// Re-export commonly used constants at module level for convenience
pub use special::ROOT_CHANGE_ID;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_change_id_is_all_z() {
        assert!(ROOT_CHANGE_ID.chars().all(|c| c == 'z'));
    }

    #[test]
    fn test_root_change_id_length() {
        // jj uses 8 character short IDs by default
        assert_eq!(ROOT_CHANGE_ID.len(), 8);
    }

    #[test]
    fn test_jj_command_name() {
        assert_eq!(JJ_COMMAND, "jj");
    }

    #[test]
    fn test_no_color_flag_format() {
        // Ensure the flag is in the correct format
        assert!(flags::NO_COLOR.starts_with("--color="));
    }
}
