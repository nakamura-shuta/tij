//! Parser for `jj bookmark list --all` output

use crate::model::Bookmark;

/// Parse `jj bookmark list --all-remotes -T ...` output
///
/// Template format: `separate("\t", name, remote, tracked) ++ "\n"`
///
/// Output examples:
/// - `main\t\ttrue` (local bookmark, tracked)
/// - `feature-x\torigin\tfalse` (untracked remote bookmark)
/// - `main\torigin\ttrue` (tracked remote bookmark)
pub fn parse_bookmark_list(output: &str) -> Vec<Bookmark> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                return None;
            }
            let name = parts[0].to_string();
            let remote = if parts[1].is_empty() {
                None
            } else {
                Some(parts[1].to_string())
            };
            let is_tracked = parts[2] == "true";
            Some(Bookmark {
                name,
                remote,
                is_tracked,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bookmark_list() {
        let output = "main\t\ttrue\nfeature-x\torigin\tfalse\n";
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
        // Line with less than 3 fields should be skipped
        let output = "incomplete\torigin\n";
        let bookmarks = parse_bookmark_list(output);
        assert!(bookmarks.is_empty());
    }

    #[test]
    fn test_filter_untracked_remotes() {
        let output = "main\t\ttrue\nmain\torigin\ttrue\nfeature\torigin\tfalse\n";
        let bookmarks = parse_bookmark_list(output);
        let untracked: Vec<_> = bookmarks
            .iter()
            .filter(|b| b.is_untracked_remote())
            .collect();
        assert_eq!(untracked.len(), 1);
        assert_eq!(untracked[0].name, "feature");
    }
}
