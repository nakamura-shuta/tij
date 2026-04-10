//! Parser for `jj workspace list` output

use crate::model::{ChangeId, WorkspaceInfo};

/// Parse `jj workspace list` output with template:
///
/// Template: `name ++ "\t" ++ self.root() ++ "\t" ++ self.target().change_id().short(8)
///            ++ "\t" ++ self.target().description().first_line() ++ "\n"`
///
/// Uses splitn(4, '\t') so that description (4th field) can safely contain tabs.
/// root_path may be an error string if workspace path is not recorded.
pub fn parse_workspace_list(output: &str) -> Vec<WorkspaceInfo> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '\t').collect();
            if parts.len() < 3 {
                return None;
            }

            let name = parts[0].to_string();
            let root_path = parse_root_path(parts[1]);
            let change_id = ChangeId::new(parts[2].to_string());
            let description = parts.get(3).unwrap_or(&"").to_string();

            Some(WorkspaceInfo {
                name,
                root_path,
                change_id,
                description,
            })
        })
        .collect()
}

/// Parse root_path, returning None if it's an error string from jj
fn parse_root_path(s: &str) -> Option<String> {
    if s.is_empty() || s.starts_with("<Error:") {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_workspace() {
        let output = "default\t/Users/user/repo\tltyxkzyp\t(no description set)\n";
        let workspaces = parse_workspace_list(output);
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].name, "default");
        assert_eq!(workspaces[0].root_path.as_deref(), Some("/Users/user/repo"));
        assert_eq!(workspaces[0].change_id.as_str(), "ltyxkzyp");
        assert_eq!(workspaces[0].description, "(no description set)");
    }

    #[test]
    fn test_parse_multiple_workspaces() {
        let output = "default\t/Users/user/repo\tltyxkzyp\t\n\
                       feature-a\t/Users/user/feature-ws\txyzpqrst\timplement feature A\n";
        let workspaces = parse_workspace_list(output);
        assert_eq!(workspaces.len(), 2);
        assert_eq!(workspaces[0].name, "default");
        assert_eq!(workspaces[1].name, "feature-a");
        assert_eq!(workspaces[1].description, "implement feature A");
    }

    #[test]
    fn test_parse_error_root_path() {
        let output =
            "default\t<Error: Workspace has no recorded path: default>\tltyxkzyp\tsome desc\n";
        let workspaces = parse_workspace_list(output);
        assert_eq!(workspaces.len(), 1);
        assert!(workspaces[0].root_path.is_none());
    }

    #[test]
    fn test_parse_empty_output() {
        let workspaces = parse_workspace_list("");
        assert!(workspaces.is_empty());
    }

    #[test]
    fn test_parse_empty_description() {
        let output = "default\t/tmp/repo\tltyxkzyp\t\n";
        let workspaces = parse_workspace_list(output);
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].description, "");
    }

    #[test]
    fn test_parse_description_with_tabs() {
        let output = "default\t/tmp/repo\tltyxkzyp\tdesc\twith\ttabs\n";
        let workspaces = parse_workspace_list(output);
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].description, "desc\twith\ttabs");
    }

    #[test]
    fn test_parse_malformed_line_skipped() {
        let output = "incomplete\n";
        let workspaces = parse_workspace_list(output);
        assert!(workspaces.is_empty());
    }
}
