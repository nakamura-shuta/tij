//! Parser for `jj tag list` output

use crate::model::{ChangeId, CommitId, TagInfo};

/// Parse `jj tag list` output with template:
///
/// Template: `separate("\t", name, if(remote, remote, ""), if(present, "true", "false"),
///            if(tracked, "true", "false"), normal_target.change_id().short(8),
///            normal_target.commit_id().short(8), normal_target.description().first_line()) ++ "\n"`
///
/// Note: jj's `separate()` skips empty fields, so field count varies:
/// - Local tag: `name\tpresent\ttracked\tchange_id\tcommit_id\tdescription` (6 fields)
/// - Remote tag: `name\tremote\tpresent\ttracked\tchange_id\tcommit_id\tdescription` (7 fields)
///
/// Additionally, description can be empty, reducing field count by 1.
pub fn parse_tag_list(output: &str) -> Vec<TagInfo> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            match parts.len() {
                // Local tag without description: name\tpresent\ttracked\tchange_id\tcommit_id
                5 => Some(TagInfo {
                    name: parts[0].to_string(),
                    remote: None,
                    present: parts[1] == "true",
                    change_id: non_empty(parts[3]).map(ChangeId::new),
                    commit_id: non_empty(parts[4]).map(CommitId::new),
                    description: None,
                }),
                // Local tag with description: name\tpresent\ttracked\tchange_id\tcommit_id\tdescription
                // OR remote tag without description: name\tremote\tpresent\ttracked\tchange_id\tcommit_id
                6 => {
                    // Distinguish by checking if parts[1] is "true"/"false" (present field for local)
                    // or a remote name
                    if parts[1] == "true" || parts[1] == "false" {
                        // Local tag with description
                        Some(TagInfo {
                            name: parts[0].to_string(),
                            remote: None,
                            present: parts[1] == "true",
                            change_id: non_empty(parts[3]).map(ChangeId::new),
                            commit_id: non_empty(parts[4]).map(CommitId::new),
                            description: non_empty(parts[5]),
                        })
                    } else {
                        // Remote tag without description
                        Some(TagInfo {
                            name: parts[0].to_string(),
                            remote: Some(parts[1].to_string()),
                            present: parts[2] == "true",
                            change_id: non_empty(parts[4]).map(ChangeId::new),
                            commit_id: non_empty(parts[5]).map(CommitId::new),
                            description: None,
                        })
                    }
                }
                // Remote tag with description: name\tremote\tpresent\ttracked\tchange_id\tcommit_id\tdescription
                7 => Some(TagInfo {
                    name: parts[0].to_string(),
                    remote: Some(parts[1].to_string()),
                    present: parts[2] == "true",
                    change_id: non_empty(parts[4]).map(ChangeId::new),
                    commit_id: non_empty(parts[5]).map(CommitId::new),
                    description: non_empty(parts[6]),
                }),
                _ => None, // Malformed line
            }
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
        let output = "v0.4.10\ttrue\tfalse\tmzslzzzz\t57d01adc\tfix: preview pane\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.4.10");
        assert!(tags[0].remote.is_none());
        assert!(tags[0].present);
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
        let output = "v0.4.10\ttrue\tfalse\tmzslzzzz\t57d01adc\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.4.10");
        assert!(tags[0].description.is_none());
    }

    #[test]
    fn test_parse_multiple_tags() {
        let output = "v0.4.10\ttrue\tfalse\tmzslzzzz\t57d01adc\tfix: something\n\
                       v0.4.9\ttrue\tfalse\tswknqzvs\t11701b8d\tfeat: highlight\n\
                       v0.4.8\ttrue\tfalse\tqknsuxln\tc902c6c0\tfix: notification\n";
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
        let output = "v0.4.10\torigin\ttrue\tfalse\tmzslzzzz\t57d01adc\tfix: something\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v0.4.10");
        assert_eq!(tags[0].remote.as_deref(), Some("origin"));
        assert!(tags[0].present);
        assert_eq!(
            tags[0].change_id.as_ref().map(|id| id.as_str()),
            Some("mzslzzzz")
        );
        assert_eq!(tags[0].description.as_deref(), Some("fix: something"));
    }

    #[test]
    fn test_parse_remote_tag_without_description() {
        let output = "v0.4.10\torigin\ttrue\tfalse\tmzslzzzz\t57d01adc\n";
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

        let output2 = "a\tb\tc\td\te\tf\tg\th\n";
        let tags2 = parse_tag_list(output2);
        assert!(tags2.is_empty());
    }

    #[test]
    fn test_parse_mixed_local_and_blank_lines() {
        let output = "v1.0\ttrue\tfalse\tabc12345\tdef67890\trelease 1.0\n\n\
                       v0.9\ttrue\tfalse\txyz98765\t12345678\tbeta\n";
        let tags = parse_tag_list(output);
        assert_eq!(tags.len(), 2);
    }
}
