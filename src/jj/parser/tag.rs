//! Parser for `jj tag list` output

use crate::model::{ChangeId, CommitId, TagInfo};

/// Number of tab-separated fields produced by the tag list template
const TAG_FIELD_COUNT: usize = 8;

/// Parse `jj tag list --all-remotes` output.
///
/// The template (see `Executor::tag_list`) concatenates fields explicitly with
/// `++ "\t" ++` rather than using `separate()`, so every row has exactly
/// [`TAG_FIELD_COUNT`] fields even when some of them are empty:
///
/// `name \t remote \t present \t tracked \t conflict \t change_id \t commit_id \t description`
///
/// Examples (`→` marks a tab):
///
/// ```text
/// vdel2→→false→false→false→→→                                  (local, target missing)
/// vdel2→origin→true→true→false→pluqrvso→59584d67→c1            (remote, tracked)
/// ```
///
/// Rows whose field count differs are malformed and get dropped.
pub fn parse_tag_list(output: &str) -> Vec<TagInfo> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() != TAG_FIELD_COUNT {
                return None; // Malformed line
            }
            Some(TagInfo {
                name: parts[0].to_string(),
                remote: non_empty(parts[1]),
                present: parts[2] == "true",
                tracked: parts[3] == "true",
                conflict: parts[4] == "true",
                change_id: non_empty(parts[5]).map(ChangeId::new),
                commit_id: non_empty(parts[6]).map(CommitId::new),
                description: non_empty(parts[7]),
            })
        })
        .collect()
}

/// Convert empty string to None
fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_local_tag_with_description() {
        let output = "v0.4.10\t\ttrue\tfalse\tfalse\tmzslzzzz\t57d01adc\tfix: preview pane\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.4.10");
        assert!(tags[0].remote.is_none());
        assert!(tags[0].present);
        assert!(!tags[0].tracked);
        assert!(!tags[0].conflict);
        assert_eq!(
            tags[0].change_id.as_ref().map(|id| id.as_str()),
            Some("mzslzzzz")
        );
        assert_eq!(
            tags[0].commit_id.as_ref().map(|id| id.as_str()),
            Some("57d01adc")
        );
        assert_eq!(tags[0].description.as_deref(), Some("fix: preview pane"));
    }

    #[test]
    fn test_parse_local_tag_without_description() {
        let output = "v0.4.10\t\ttrue\tfalse\tfalse\tmzslzzzz\t57d01adc\t\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.4.10");
        assert!(tags[0].description.is_none());
    }

    #[test]
    fn test_parse_multiple_tags() {
        let output = "v0.4.10\t\ttrue\tfalse\tfalse\tmzslzzzz\t57d01adc\tfix: something\n\
                       v0.4.9\t\ttrue\tfalse\tfalse\tswknqzvs\t11701b8d\tfeat: highlight\n\
                       v0.4.8\t\ttrue\tfalse\tfalse\tqknsuxln\tc902c6c0\tfix: notification\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0].name, "v0.4.10");
        assert_eq!(tags[1].name, "v0.4.9");
        assert_eq!(tags[2].name, "v0.4.8");
    }

    #[test]
    fn test_parse_empty_output() {
        let tags = parse_tag_list("");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_remote_tag_with_description() {
        let output = "v0.4.10\torigin\ttrue\ttrue\tfalse\tmzslzzzz\t57d01adc\tfix: something\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.4.10");
        assert_eq!(tags[0].remote.as_deref(), Some("origin"));
        assert!(tags[0].present);
        assert!(tags[0].tracked);
        assert_eq!(
            tags[0].change_id.as_ref().map(|id| id.as_str()),
            Some("mzslzzzz")
        );
        assert_eq!(tags[0].description.as_deref(), Some("fix: something"));
    }

    #[test]
    fn test_parse_remote_tag_without_description() {
        let output = "v0.4.10\torigin\ttrue\ttrue\tfalse\tmzslzzzz\t57d01adc\t\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.4.10");
        assert_eq!(tags[0].remote.as_deref(), Some("origin"));
        assert!(tags[0].description.is_none());
    }

    #[test]
    fn test_parse_malformed_line_skipped() {
        let output = "incomplete\n";
        let tags = parse_tag_list(output);
        assert!(tags.is_empty());

        // 9 fields is malformed too
        let output2 = "a\tb\tc\td\te\tf\tg\th\ti\n";
        let tags2 = parse_tag_list(output2);
        assert!(tags2.is_empty());
    }

    #[test]
    fn test_parse_mixed_local_and_blank_lines() {
        let output = "v1.0\t\ttrue\tfalse\tfalse\tabc12345\tdef67890\trelease 1.0\n\n\
                       v0.9\t\ttrue\tfalse\tfalse\txyz98765\t12345678\tbeta\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 2);
    }

    // ── New format: fixed 8 columns ────────────────────────────────

    #[test]
    fn test_parse_local_row_has_eight_fields() {
        // Local rows report tracked=false; the remote column is empty
        let output = "v1.0\t\ttrue\tfalse\tfalse\tabc12345\tdef67890\trelease 1.0\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert!(tags[0].is_local());
        assert!(!tags[0].is_untracked_remote());
        assert!(!tags[0].is_tracked_remote());
    }

    #[test]
    fn test_parse_remote_tracked_row() {
        let output = "v1.0\torigin\ttrue\ttrue\tfalse\tabc12345\tdef67890\trelease 1.0\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].remote.as_deref(), Some("origin"));
        assert!(tags[0].tracked);
        assert!(tags[0].is_tracked_remote());
        assert!(!tags[0].is_untracked_remote());
        assert_eq!(tags[0].full_name(), "v1.0@origin");
    }

    #[test]
    fn test_parse_remote_untracked_row() {
        let output = "v0.9.6\torigin\ttrue\tfalse\tfalse\tabc12345\tdef67890\told release\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].remote.as_deref(), Some("origin"));
        assert!(!tags[0].tracked);
        assert!(tags[0].is_untracked_remote());
        assert!(!tags[0].is_tracked_remote());
    }

    #[test]
    fn test_parse_conflict_row() {
        let output = "v1.0\torigin\ttrue\ttrue\ttrue\tabc12345\tdef67890\trelease 1.0\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert!(tags[0].conflict);
    }

    #[test]
    fn test_parse_row_with_all_targets_missing_is_kept() {
        // `present == false` row: local tag deleted while the remote tag remains.
        // `try(..., "")` in the template empties the three target columns, so the
        // row still has 8 fields and MUST NOT be dropped.
        let output = "vdel2\t\tfalse\tfalse\tfalse\t\t\t\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "vdel2");
        assert!(tags[0].remote.is_none());
        assert!(!tags[0].present);
        assert!(!tags[0].tracked);
        assert!(!tags[0].conflict);
        assert!(tags[0].change_id.is_none());
        assert!(tags[0].commit_id.is_none());
        assert!(tags[0].description.is_none());
        assert!(!tags[0].is_jumpable());
    }

    #[test]
    fn test_parse_present_false_pair_from_real_output() {
        // Both rows verified against jj 0.44 with `try(..., "")` in place.
        let output = "vdel2\t\tfalse\tfalse\tfalse\t\t\t\n\
                      vdel2\torigin\ttrue\ttrue\tfalse\tpluqrvso\t59584d67\tc1\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 2);
        assert!(!tags[0].present);
        assert!(tags[1].present);
        assert!(tags[1].is_tracked_remote());
    }

    #[test]
    fn test_parse_seven_or_fewer_fields_dropped() {
        // The old (pre-0.44) `separate()` formats produced 5/6/7 columns.
        // They must all be rejected now, so a stale template fails loudly.
        for output in [
            "v1.0\ttrue\tfalse\tabc12345\tdef67890\n",
            "v1.0\ttrue\tfalse\tabc12345\tdef67890\trelease 1.0\n",
            "v1.0\torigin\ttrue\tfalse\tabc12345\tdef67890\trelease 1.0\n",
        ] {
            let tags = parse_tag_list(output);
            assert!(
                tags.is_empty(),
                "expected {output:?} to be dropped, got {tags:?}"
            );
        }
    }

    #[test]
    fn test_parse_git_internal_remote_row_is_kept_by_parser() {
        // Colocated repos yield a `git` remote row. Filtering it out is the
        // View's job, not the parser's.
        let output = "v1.0\tgit\ttrue\ttrue\tfalse\tabc12345\tdef67890\trelease 1.0\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].remote.as_deref(), Some("git"));
    }
}
