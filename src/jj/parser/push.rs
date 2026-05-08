//! Push dry-run output parser
//!
//! Parses the output of `jj git push --dry-run` into structured data.

/// Parsed action from a dry-run push preview
#[derive(Debug, Clone, PartialEq)]
pub enum PushPreviewAction {
    /// Move forward bookmark from old_hash to new_hash (safe fast-forward)
    MoveForward {
        bookmark: String,
        from: String,
        to: String,
    },
    /// Move sideways bookmark (diverged: e.g. after rebase) — force push
    MoveSideways {
        bookmark: String,
        from: String,
        to: String,
    },
    /// Move backward bookmark (regression: e.g. after reset) — force push
    MoveBackward {
        bookmark: String,
        from: String,
        to: String,
    },
    /// Add new bookmark at hash
    Add { bookmark: String, to: String },
    /// Delete bookmark at hash
    Delete { bookmark: String, from: String },
}

/// Result of parsing `jj git push --dry-run` output
///
/// Only used when `git_push_dry_run()` returns `Ok(output)` (exit 0).
/// Error cases (exit != 0) are handled by the caller's `Err(_)` branch.
#[derive(Debug, Clone, PartialEq)]
pub enum PushPreviewResult {
    /// Changes to push
    Changes(Vec<PushPreviewAction>),
    /// Already up to date (no changes needed)
    NothingChanged,
    /// Output could not be parsed (unknown format from newer jj version, etc.)
    Unparsed,
}

/// Parse the output of `jj git push --dry-run` (exit 0 only)
///
/// Expected output patterns:
/// - `Move forward bookmark NAME from HASH to HASH`
/// - `Move sideways bookmark NAME from HASH to HASH` (force push)
/// - `Move backward bookmark NAME from HASH to HASH` (force push)
/// - `Add bookmark NAME to HASH`
/// - `Delete bookmark NAME from HASH`
/// - `Nothing changed.` (when bookmark is already up to date)
pub fn parse_push_dry_run(output: &str) -> PushPreviewResult {
    if output.contains("Nothing changed.") {
        return PushPreviewResult::NothingChanged;
    }

    let mut actions = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Move forward bookmark ") {
            // "Move forward bookmark NAME from HASH to HASH"
            if let Some((name, hashes)) = rest.split_once(" from ")
                && let Some((from, to)) = hashes.split_once(" to ")
            {
                actions.push(PushPreviewAction::MoveForward {
                    bookmark: name.to_string(),
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
        } else if let Some(rest) = line.strip_prefix("Move sideways bookmark ") {
            // "Move sideways bookmark NAME from HASH to HASH" (force push)
            if let Some((name, hashes)) = rest.split_once(" from ")
                && let Some((from, to)) = hashes.split_once(" to ")
            {
                actions.push(PushPreviewAction::MoveSideways {
                    bookmark: name.to_string(),
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
        } else if let Some(rest) = line.strip_prefix("Move backward bookmark ") {
            // "Move backward bookmark NAME from HASH to HASH" (force push)
            if let Some((name, hashes)) = rest.split_once(" from ")
                && let Some((from, to)) = hashes.split_once(" to ")
            {
                actions.push(PushPreviewAction::MoveBackward {
                    bookmark: name.to_string(),
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
        } else if let Some(rest) = line.strip_prefix("Add bookmark ") {
            // "Add bookmark NAME to HASH"
            if let Some((name, hash)) = rest.split_once(" to ") {
                actions.push(PushPreviewAction::Add {
                    bookmark: name.to_string(),
                    to: hash.to_string(),
                });
            }
        } else if let Some(rest) = line.strip_prefix("Delete bookmark ") {
            // "Delete bookmark NAME from HASH"
            if let Some((name, hash)) = rest.split_once(" from ") {
                actions.push(PushPreviewAction::Delete {
                    bookmark: name.to_string(),
                    from: hash.to_string(),
                });
            }
        }
        // "Changes to push to origin:" and "Dry-run requested, not pushing." are ignored
    }

    if actions.is_empty() {
        PushPreviewResult::Unparsed
    } else {
        PushPreviewResult::Changes(actions)
    }
}

/// Parsed entry for a bookmark/tag that jj 0.41+ skipped during a bulk push.
///
/// Format from jj source (`cli/src/commands/git/push.rs`):
/// `Won't push <kind> <name>: commit <id> <message>`
/// where `<message>` is e.g. "is private" or "has conflicts".
#[derive(Debug, Clone, PartialEq)]
pub struct SkippedRef {
    pub name: String,
    pub reason: String,
}

/// Scan jj push stderr for "Won't push" warning lines.
///
/// Returns one entry per skipped bookmark/tag in the order encountered.
/// jj 0.41 made `jj git push --all/--tracked/-r` skip rather than fail when
/// some refs are private or have conflicts; the parsed list lets the caller
/// surface "Pushed N, skipped M" notifications.
pub fn parse_push_skipped(stderr: &str) -> Vec<SkippedRef> {
    stderr
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("Won't push ")?;
            let (_kind, after_kind) = rest.split_once(' ')?;
            let (name_part, reason_part) = after_kind.split_once(": ")?;
            let name = name_part.trim().trim_matches('"').to_string();
            let reason = reason_part.trim().to_string();
            Some(SkippedRef { name, reason })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_move_forward() {
        let output = "Changes to push to origin:\n  Move forward bookmark main from 6c733e1ae096 to f70230817ff4\nDry-run requested, not pushing.\n";
        let result = parse_push_dry_run(output);
        match result {
            PushPreviewResult::Changes(actions) => {
                assert_eq!(actions.len(), 1);
                assert_eq!(
                    actions[0],
                    PushPreviewAction::MoveForward {
                        bookmark: "main".to_string(),
                        from: "6c733e1ae096".to_string(),
                        to: "f70230817ff4".to_string(),
                    }
                );
            }
            _ => panic!("Expected Changes"),
        }
    }

    #[test]
    fn test_parse_add_bookmark() {
        let output = "Changes to push to origin:\n  Add bookmark feature/new to f70230817ff4\nDry-run requested, not pushing.\n";
        let result = parse_push_dry_run(output);
        match result {
            PushPreviewResult::Changes(actions) => {
                assert_eq!(actions.len(), 1);
                assert_eq!(
                    actions[0],
                    PushPreviewAction::Add {
                        bookmark: "feature/new".to_string(),
                        to: "f70230817ff4".to_string(),
                    }
                );
            }
            _ => panic!("Expected Changes"),
        }
    }

    #[test]
    fn test_parse_delete_bookmark() {
        let output = "Changes to push to origin:\n  Delete bookmark old-branch from 6c733e1ae096\nDry-run requested, not pushing.\n";
        let result = parse_push_dry_run(output);
        match result {
            PushPreviewResult::Changes(actions) => {
                assert_eq!(actions.len(), 1);
                assert_eq!(
                    actions[0],
                    PushPreviewAction::Delete {
                        bookmark: "old-branch".to_string(),
                        from: "6c733e1ae096".to_string(),
                    }
                );
            }
            _ => panic!("Expected Changes"),
        }
    }

    #[test]
    fn test_parse_multiple_changes() {
        let output = "Changes to push to origin:\n  Move forward bookmark another-branch from 6c733e1ae096 to f70230817ff4\n  Add bookmark fuga to bfeefc809de1\n  Add bookmark main to f70230817ff4\nDry-run requested, not pushing.\n";
        let result = parse_push_dry_run(output);
        match result {
            PushPreviewResult::Changes(actions) => {
                assert_eq!(actions.len(), 3);
                assert!(matches!(&actions[0], PushPreviewAction::MoveForward { .. }));
                assert!(
                    matches!(&actions[1], PushPreviewAction::Add { bookmark, .. } if bookmark == "fuga")
                );
                assert!(
                    matches!(&actions[2], PushPreviewAction::Add { bookmark, .. } if bookmark == "main")
                );
            }
            _ => panic!("Expected Changes"),
        }
    }

    #[test]
    fn test_parse_nothing_changed() {
        let output =
            "Bookmark test-feature@origin already matches test-feature\nNothing changed.\n";
        let result = parse_push_dry_run(output);
        assert_eq!(result, PushPreviewResult::NothingChanged);
    }

    #[test]
    fn test_parse_empty_output() {
        let result = parse_push_dry_run("");
        assert_eq!(result, PushPreviewResult::Unparsed);
    }

    #[test]
    fn test_parse_unknown_output() {
        // Unknown jj output format should return Unparsed, not NothingChanged
        let result = parse_push_dry_run("Some unexpected jj output format\n");
        assert_eq!(result, PushPreviewResult::Unparsed);
    }

    #[test]
    fn test_parse_move_sideways() {
        let output = "Changes to push to origin:\n  Move sideways bookmark feature from 6c733e1ae096 to f70230817ff4\nDry-run requested, not pushing.\n";
        let result = parse_push_dry_run(output);
        match result {
            PushPreviewResult::Changes(actions) => {
                assert_eq!(actions.len(), 1);
                assert_eq!(
                    actions[0],
                    PushPreviewAction::MoveSideways {
                        bookmark: "feature".to_string(),
                        from: "6c733e1ae096".to_string(),
                        to: "f70230817ff4".to_string(),
                    }
                );
            }
            _ => panic!("Expected Changes"),
        }
    }

    #[test]
    fn test_parse_move_backward() {
        let output = "Changes to push to origin:\n  Move backward bookmark main from f70230817ff4 to 6c733e1ae096\nDry-run requested, not pushing.\n";
        let result = parse_push_dry_run(output);
        match result {
            PushPreviewResult::Changes(actions) => {
                assert_eq!(actions.len(), 1);
                assert_eq!(
                    actions[0],
                    PushPreviewAction::MoveBackward {
                        bookmark: "main".to_string(),
                        from: "f70230817ff4".to_string(),
                        to: "6c733e1ae096".to_string(),
                    }
                );
            }
            _ => panic!("Expected Changes"),
        }
    }

    #[test]
    fn test_parse_mixed_actions_with_force() {
        let output = "Changes to push to origin:\n  Move forward bookmark main from 6c733e1ae096 to f70230817ff4\n  Move sideways bookmark feature from aaa111bbb222 to ccc333ddd444\nDry-run requested, not pushing.\n";
        let result = parse_push_dry_run(output);
        match result {
            PushPreviewResult::Changes(actions) => {
                assert_eq!(actions.len(), 2);
                assert!(
                    matches!(&actions[0], PushPreviewAction::MoveForward { bookmark, .. } if bookmark == "main")
                );
                assert!(
                    matches!(&actions[1], PushPreviewAction::MoveSideways { bookmark, .. } if bookmark == "feature")
                );
            }
            _ => panic!("Expected Changes"),
        }
    }

    #[test]
    fn test_parse_push_skipped_empty() {
        assert!(parse_push_skipped("").is_empty());
        assert!(parse_push_skipped("Changes to push to origin:\n").is_empty());
    }

    #[test]
    fn test_parse_push_skipped_private_bookmark() {
        let stderr = "Won't push bookmark feature: commit abc123 is private\n";
        let skipped = parse_push_skipped(stderr);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].name, "feature");
        assert_eq!(skipped[0].reason, "commit abc123 is private");
    }

    #[test]
    fn test_parse_push_skipped_multiple() {
        let stderr = "Won't push bookmark foo: commit aaa is private\n\
                      Changes to push to origin:\n  \
                      Move forward bookmark main from 111 to 222\n\
                      Won't push bookmark bar: commit bbb has conflicts\n\
                      Won't push tag v1: commit ccc has conflicts\n";
        let skipped = parse_push_skipped(stderr);
        assert_eq!(skipped.len(), 3);
        assert_eq!(skipped[0].name, "foo");
        assert_eq!(skipped[1].name, "bar");
        assert_eq!(skipped[2].name, "v1");
    }

    #[test]
    fn test_parse_push_skipped_quoted_name() {
        let stderr = "Won't push bookmark \"feat/x\": commit abc has conflicts\n";
        let skipped = parse_push_skipped(stderr);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].name, "feat/x");
    }
}
