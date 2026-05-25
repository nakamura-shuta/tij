//! Parser for `jj bookmark list --all-remotes` output

use crate::model::Bookmark;

/// Parse `jj bookmark list --all-remotes -T ...` output
///
/// Template format: `separate("\t", name, remote, tracked) ++ "\n"`
///
/// Note: jj's `separate()` skips empty fields, so output varies:
/// - Local bookmark: `name\ttracked` (2 fields, remote is empty/skipped)
/// - Remote bookmark: `name\tremote\ttracked` (3 fields)
///
/// Output examples:
/// - `main\tfalse` (local bookmark, 2 fields)
/// - `feature-x\torigin\tfalse` (untracked remote bookmark, 3 fields)
/// - `main\torigin\ttrue` (tracked remote bookmark, 3 fields)
pub fn parse_bookmark_list(output: &str) -> Vec<Bookmark> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            match parts.len() {
                2 => {
                    // Local bookmark: name\ttracked
                    let name = parts[0].to_string();
                    let is_tracked = parts[1] == "true";
                    Some(Bookmark {
                        name,
                        remote: None,
                        is_tracked,
                    })
                }
                3 => {
                    // Remote bookmark: name\tremote\ttracked
                    let name = parts[0].to_string();
                    let remote = Some(parts[1].to_string());
                    let is_tracked = parts[2] == "true";
                    Some(Bookmark {
                        name,
                        remote,
                        is_tracked,
                    })
                }
                _ => None, // Malformed line
            }
        })
        .collect()
}

/// Parse `jj log -T 'bookmarks.map(|x| x.name()).join(" ")'` output into a
/// de-duplicated, order-preserving list of bookmark names.
///
/// Each line is one commit; within a line names are space-separated.
pub fn parse_advance_bookmarks(output: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in output.lines() {
        for name in line.split_whitespace() {
            let name = name.to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_advance_bookmarks_single() {
        assert_eq!(parse_advance_bookmarks("main\n"), vec!["main".to_string()]);
    }

    #[test]
    fn test_parse_advance_bookmarks_multiple_lines() {
        assert_eq!(
            parse_advance_bookmarks("main\nrelease\n"),
            vec!["main".to_string(), "release".to_string()]
        );
    }

    #[test]
    fn test_parse_advance_bookmarks_multiple_on_one_commit() {
        assert_eq!(
            parse_advance_bookmarks("main release\n"),
            vec!["main".to_string(), "release".to_string()]
        );
    }

    #[test]
    fn test_parse_advance_bookmarks_empty() {
        assert!(parse_advance_bookmarks("").is_empty());
        assert!(parse_advance_bookmarks("\n\n").is_empty());
    }

    #[test]
    fn test_parse_advance_bookmarks_dedup() {
        assert_eq!(
            parse_advance_bookmarks("main\nmain\n"),
            vec!["main".to_string()]
        );
    }

    #[test]
    fn test_parse_bookmark_list() {
        // jj's separate() skips empty fields, so:
        // - Local bookmark: name\ttracked (2 fields)
        // - Remote bookmark: name\tremote\ttracked (3 fields)
        let output = "main\ttrue\nfeature-x\torigin\tfalse\n";
        let bookmarks = parse_bookmark_list(output);
        assert_eq!(bookmarks.len(), 2);
        assert_eq!(bookmarks[0].name, "main");
        assert!(bookmarks[0].is_tracked);
        assert!(bookmarks[0].remote.is_none());
        assert_eq!(bookmarks[1].full_name(), "feature-x@origin");
        assert!(!bookmarks[1].is_tracked);
    }

    #[test]
    fn test_parse_tracked_remote() {
        // Remote bookmark that is tracked (main@origin after tracking)
        let output = "main\torigin\ttrue\n";
        let bookmarks = parse_bookmark_list(output);
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].name, "main");
        assert_eq!(bookmarks[0].remote, Some("origin".to_string()));
        assert!(bookmarks[0].is_tracked);
        assert!(!bookmarks[0].is_untracked_remote()); // tracked, so not "untracked remote"
    }

    #[test]
    fn test_parse_empty_output() {
        let bookmarks = parse_bookmark_list("");
        assert!(bookmarks.is_empty());
    }

    #[test]
    fn test_parse_malformed_line() {
        // Line with only 1 field should be skipped
        let output = "incomplete\n";
        let bookmarks = parse_bookmark_list(output);
        assert!(bookmarks.is_empty());

        // Line with 4+ fields should also be skipped
        let output2 = "name\tremote\ttrue\textra\n";
        let bookmarks2 = parse_bookmark_list(output2);
        assert!(bookmarks2.is_empty());
    }

    #[test]
    fn test_filter_untracked_remotes() {
        // jj's separate() skips empty fields:
        // - main (local, tracked): 2 fields
        // - main@origin (tracked remote): 3 fields
        // - feature@origin (untracked remote): 3 fields
        let output = "main\ttrue\nmain\torigin\ttrue\nfeature\torigin\tfalse\n";
        let bookmarks = parse_bookmark_list(output);
        assert_eq!(bookmarks.len(), 3);
        let untracked: Vec<_> = bookmarks
            .iter()
            .filter(|b| b.is_untracked_remote())
            .collect();
        assert_eq!(untracked.len(), 1);
        assert_eq!(untracked[0].name, "feature");
    }
}
