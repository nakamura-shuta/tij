//! Parser for `jj evolog` output

use crate::model::EvologEntry;

/// Parse `jj evolog` tab-separated output into EvologEntry list
pub fn parse_evolog(output: &str) -> Vec<EvologEntry> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(6, '\t').collect();
            if parts.len() < 6 {
                return None;
            }
            Some(EvologEntry {
                commit_id: parts[0].to_string(),
                change_id: parts[1].to_string(),
                author: parts[2].to_string(),
                timestamp: parts[3].to_string(),
                is_empty: parts[4] == "[empty]",
                description: parts[5].to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_evolog_normal() {
        let output = "43a4bc7d\tzxsrvopz\tuser@example.com\t2025-10-03 18:10:00\t\tmy feature description\n\
                       7aa68914\tzxsrvopz\tuser@example.com\t2025-10-03 18:08:05\t\t(no description set)\n";
        let entries = parse_evolog(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].commit_id, "43a4bc7d");
        assert_eq!(entries[0].change_id, "zxsrvopz");
        assert_eq!(entries[0].author, "user@example.com");
        assert_eq!(entries[0].timestamp, "2025-10-03 18:10:00");
        assert!(!entries[0].is_empty);
        assert_eq!(entries[0].description, "my feature description");

        assert_eq!(entries[1].commit_id, "7aa68914");
        assert_eq!(entries[1].description, "(no description set)");
    }

    #[test]
    fn test_parse_evolog_empty_commit() {
        let output = "initial1\tzxsrvopz\tuser@example.com\t2025-10-03 18:05:00\t[empty]\t(no description set)\n";
        let entries = parse_evolog(output);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_empty);
    }

    #[test]
    fn test_parse_evolog_empty_output() {
        let entries = parse_evolog("");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_evolog_description_with_tab() {
        let output =
            "abc12345\tzxsrvopz\tuser@example.com\t2025-10-03 18:10:00\t\tdesc\twith\ttabs\n";
        let entries = parse_evolog(output);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].description, "desc\twith\ttabs");
    }

    #[test]
    fn test_parse_evolog_malformed_line_skipped() {
        let output = "only\ttwo\tfields\n\
                       43a4bc7d\tzxsrvopz\tuser@example.com\t2025-10-03 18:10:00\t\tdescription\n";
        let entries = parse_evolog(output);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].commit_id, "43a4bc7d");
    }
}
