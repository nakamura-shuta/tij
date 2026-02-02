//! jj-specific constants
//!
//! Centralized definitions for jj command names, flags, and special values.

/// jj command binary name
pub const JJ_COMMAND: &str = "jj";

/// Minimum supported jj version
pub const MIN_JJ_VERSION: &str = "0.20.0";

/// jj subcommands
pub mod commands {
    pub const LOG: &str = "log";
    pub const STATUS: &str = "status";
    pub const DIFF: &str = "diff";
    pub const SHOW: &str = "show";
    pub const DESCRIBE: &str = "describe";
    pub const NEW: &str = "new";
    pub const EDIT: &str = "edit";
    pub const COMMIT: &str = "commit";
    pub const UNDO: &str = "undo";
    pub const OP: &str = "op";
    pub const OP_LOG: &str = "log";
    pub const OP_RESTORE: &str = "restore";
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
    /// Show version
    pub const VERSION: &str = "--version";
}

/// Special jj values
pub mod special {
    /// The root change ID (all 'z' characters)
    ///
    /// In jj, the root commit has a special change ID consisting of all 'z'.
    /// This is used to identify and specially render the root in Log View.
    pub const ROOT_CHANGE_ID: &str = "zzzzzzzz";

    /// Version output prefix (e.g., "jj 0.37.0")
    pub const VERSION_PREFIX: &str = "jj ";
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
