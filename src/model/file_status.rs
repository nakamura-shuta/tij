//! File status data model

/// Overall repository status
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// List of changed files
    pub files: Vec<FileStatus>,

    /// Are there any conflicted files?
    pub has_conflicts: bool,

    /// Current working copy change ID
    pub working_copy_change_id: String,

    /// Parent change ID
    pub parent_change_id: String,
}

impl Status {
    /// Check if the working copy is clean (no changes)
    pub fn is_clean(&self) -> bool {
        self.files.is_empty()
    }

    /// Get count of files by state
    pub fn count_by_state(&self, state: &FileState) -> usize {
        self.files
            .iter()
            .filter(|f| std::mem::discriminant(&f.state) == std::mem::discriminant(state))
            .count()
    }
}

/// Status of a single file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStatus {
    /// File path
    pub path: String,

    /// State of the file
    pub state: FileState,
}

impl FileStatus {
    /// Get the status indicator character
    pub fn indicator(&self) -> char {
        match &self.state {
            FileState::Added => 'A',
            FileState::Modified => 'M',
            FileState::Deleted => 'D',
            FileState::Renamed { .. } => 'R',
            FileState::Conflicted => 'C',
        }
    }
}

/// Possible states for a file
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    /// File was added
    Added,

    /// File was modified
    Modified,

    /// File was deleted
    Deleted,

    /// File was renamed
    Renamed {
        /// Original path
        from: String,
    },

    /// File has conflicts
    Conflicted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_is_clean() {
        let clean = Status {
            files: vec![],
            has_conflicts: false,
            working_copy_change_id: "abc".to_string(),
            parent_change_id: "xyz".to_string(),
        };
        assert!(clean.is_clean());

        let dirty = Status {
            files: vec![FileStatus {
                path: "test.rs".to_string(),
                state: FileState::Modified,
            }],
            has_conflicts: false,
            working_copy_change_id: "abc".to_string(),
            parent_change_id: "xyz".to_string(),
        };
        assert!(!dirty.is_clean());
    }

    #[test]
    fn test_status_count_by_state() {
        let status = Status {
            files: vec![
                FileStatus {
                    path: "a.rs".to_string(),
                    state: FileState::Added,
                },
                FileStatus {
                    path: "b.rs".to_string(),
                    state: FileState::Modified,
                },
                FileStatus {
                    path: "c.rs".to_string(),
                    state: FileState::Added,
                },
            ],
            has_conflicts: false,
            working_copy_change_id: "abc".to_string(),
            parent_change_id: "xyz".to_string(),
        };

        assert_eq!(status.count_by_state(&FileState::Added), 2);
        assert_eq!(status.count_by_state(&FileState::Modified), 1);
        assert_eq!(status.count_by_state(&FileState::Deleted), 0);
    }

    #[test]
    fn test_file_status_indicator() {
        assert_eq!(
            FileStatus {
                path: "a".to_string(),
                state: FileState::Added
            }
            .indicator(),
            'A'
        );
        assert_eq!(
            FileStatus {
                path: "b".to_string(),
                state: FileState::Modified
            }
            .indicator(),
            'M'
        );
        assert_eq!(
            FileStatus {
                path: "c".to_string(),
                state: FileState::Deleted
            }
            .indicator(),
            'D'
        );
        assert_eq!(
            FileStatus {
                path: "d".to_string(),
                state: FileState::Renamed {
                    from: "old".to_string()
                }
            }
            .indicator(),
            'R'
        );
        assert_eq!(
            FileStatus {
                path: "e".to_string(),
                state: FileState::Conflicted
            }
            .indicator(),
            'C'
        );
    }
}
