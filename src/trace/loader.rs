//! Tolerant JSONL loader for Agent Trace sidecar files
//!
//! Reads `.agent-trace/traces.jsonl` (the reference implementation's
//! append-only sidecar). Parsing is deliberately forgiving (§5.2 of the SoW):
//! each line is parsed as a generic `serde_json::Value`, then fields are
//! extracted one by one — a type-mismatched field drops only that field (or
//! array element), never the whole record. Whole lines are skipped only when
//! they are not valid JSON objects at all.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde_json::Value;

use super::model::{
    ContributorKind, TraceContributor, TraceConversation, TraceFile, TraceRange, TraceRecord,
    TraceVcs, TraceVcsType,
};

/// Default sidecar path, relative to the jj workspace root
/// (override via jj config key `tij.agent-trace.path`)
pub const DEFAULT_TRACE_PATH: &str = ".agent-trace/traces.jsonl";

/// Maximum bytes read from the trace file. Larger files are read tail-first
/// (the file is append-only, so the tail holds the newest records).
const MAX_TRACE_BYTES: u64 = 5 * 1024 * 1024;

/// Result of loading a trace file
#[derive(Debug, Default)]
pub struct LoadResult {
    pub records: Vec<TraceRecord>,
    /// True when the file exceeded [`MAX_TRACE_BYTES`] and older records
    /// were dropped
    pub truncated: bool,
}

/// Load and parse a trace sidecar file.
///
/// Returns `None` when the file does not exist or cannot be read — trace
/// problems must never surface as errors in tij (SoW principle P5/P6).
pub fn load(path: &Path) -> Option<LoadResult> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();

    let truncated = len > MAX_TRACE_BYTES;
    if truncated {
        file.seek(SeekFrom::Start(len - MAX_TRACE_BYTES)).ok()?;
    }
    let mut text = String::new();
    // Non-UTF-8 content: read_to_string fails → treat as unreadable
    file.read_to_string(&mut text).ok()?;

    let body = if truncated {
        skip_partial_first_line(&text)
    } else {
        text.as_str()
    };

    Some(LoadResult {
        records: parse_jsonl(body),
        truncated,
    })
}

/// Drop everything up to and including the first newline (a tail-read starts
/// mid-line; the partial first line is not valid JSON).
fn skip_partial_first_line(text: &str) -> &str {
    match text.find('\n') {
        Some(pos) => &text[pos + 1..],
        None => "",
    }
}

/// Parse JSONL text into records, skipping unparsable lines.
fn parse_jsonl(text: &str) -> Vec<TraceRecord> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|v| parse_record(&v))
        .collect()
}

/// Convert one JSON value into a TraceRecord (None only for non-objects).
fn parse_record(v: &Value) -> Option<TraceRecord> {
    let obj = v.as_object()?;

    let vcs = obj.get("vcs").and_then(parse_vcs);
    let (tool_name, tool_version) = obj
        .get("tool")
        .map(|t| (str_field(t, "name"), str_field(t, "version")))
        .unwrap_or((None, None));

    let files = obj
        .get("files")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_file).collect())
        .unwrap_or_default();

    Some(TraceRecord {
        timestamp: str_field(v, "timestamp").unwrap_or_default(),
        vcs,
        tool_name,
        tool_version,
        files,
    })
}

fn parse_vcs(v: &Value) -> Option<TraceVcs> {
    let revision = str_field(v, "revision")?;
    let vcs_type = match str_field(v, "type")?.as_str() {
        "git" => TraceVcsType::Git,
        "jj" => TraceVcsType::Jj,
        _ => TraceVcsType::Other,
    };
    Some(TraceVcs { vcs_type, revision })
}

fn parse_file(v: &Value) -> Option<TraceFile> {
    let path = str_field(v, "path")?;
    let conversations = v
        .get("conversations")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(parse_conversation).collect())
        .unwrap_or_default();
    Some(TraceFile {
        path,
        conversations,
    })
}

fn parse_conversation(v: &Value) -> TraceConversation {
    TraceConversation {
        url: str_field(v, "url"),
        contributor: v.get("contributor").and_then(parse_contributor),
        ranges: v
            .get("ranges")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(parse_range).collect())
            .unwrap_or_default(),
    }
}

fn parse_contributor(v: &Value) -> Option<TraceContributor> {
    let kind = match str_field(v, "type")?.as_str() {
        "human" => ContributorKind::Human,
        "ai" => ContributorKind::Ai,
        "mixed" => ContributorKind::Mixed,
        _ => ContributorKind::Unknown,
    };
    Some(TraceContributor {
        kind,
        model_id: str_field(v, "model_id"),
    })
}

fn parse_range(v: &Value) -> Option<TraceRange> {
    let start_line = usize_field(v, "start_line")?;
    let end_line = usize_field(v, "end_line")?;
    if start_line < 1 || end_line < start_line {
        return None;
    }
    Some(TraceRange {
        start_line,
        end_line,
        contributor: v.get("contributor").and_then(parse_contributor),
    })
}

/// Extract a string field (non-string values → None)
fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(str::to_string)
}

/// Extract a positive integer field (non-numbers / floats / negatives → None)
fn usize_field(v: &Value, key: &str) -> Option<usize> {
    v.get(key)?.as_u64().map(|n| n as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_one(line: &str) -> Option<TraceRecord> {
        let mut records = parse_jsonl(line);
        if records.is_empty() {
            None
        } else {
            Some(records.remove(0))
        }
    }

    /// A realistic, fully-populated record (reference-implementation shape)
    const FULL_RECORD: &str = r#"{"version":"0.1.0","id":"a1b2c3","timestamp":"2026-06-05T14:20:00Z","vcs":{"type":"jj","revision":"xqnktzmlworukplnyrropmtzylsuxxlv"},"tool":{"name":"claude-code","version":"2.0"},"files":[{"path":"src/main.rs","conversations":[{"url":"https://example.com/s/1","contributor":{"type":"ai","model_id":"anthropic/claude-opus-4-8"},"ranges":[{"start_line":1,"end_line":10}]}]}]}"#;

    #[test]
    fn parses_full_record() {
        let r = parse_one(FULL_RECORD).unwrap();
        assert_eq!(r.timestamp, "2026-06-05T14:20:00Z");
        let vcs = r.vcs.as_ref().unwrap();
        assert_eq!(vcs.vcs_type, TraceVcsType::Jj);
        assert!(vcs.revision.starts_with("xqnktzml"));
        assert_eq!(r.tool_name.as_deref(), Some("claude-code"));
        assert_eq!(r.files.len(), 1);
        let conv = &r.files[0].conversations[0];
        assert_eq!(conv.url.as_deref(), Some("https://example.com/s/1"));
        assert_eq!(conv.contributor.as_ref().unwrap().kind, ContributorKind::Ai);
        assert_eq!(conv.ranges[0].start_line, 1);
        assert!(r.has_ai_contribution());
    }

    #[test]
    fn tolerates_schema_violations_from_real_writers() {
        // version "1.0" (semver violation) + missing tool.version
        let line = r#"{"version":"1.0","timestamp":"2026-06-05T00:00:00Z","vcs":{"type":"git","revision":"a6b2ed5ac3b509694c746a4763b97995f395172b"},"tool":{"name":"claude-code"},"files":[{"path":"src/lib.rs","conversations":[{"ranges":[{"start_line":5,"end_line":8}]}]}]}"#;
        let r = parse_one(line).unwrap();
        assert_eq!(r.vcs.as_ref().unwrap().vcs_type, TraceVcsType::Git);
        assert_eq!(r.tool_name.as_deref(), Some("claude-code"));
        assert_eq!(r.tool_version, None);
        // No contributor anywhere + tool present → AI contribution (§5.3)
        assert!(r.has_ai_contribution());
    }

    #[test]
    fn type_mismatch_drops_field_not_record() {
        // start_line as string → that range is dropped, record survives
        let line = r#"{"timestamp":"t","vcs":{"type":"jj","revision":"abcdefgh12345678"},"tool":{"name":"x"},"files":[{"path":"a.rs","conversations":[{"ranges":[{"start_line":"five","end_line":8},{"start_line":2,"end_line":3}]}]}]}"#;
        let r = parse_one(line).unwrap();
        let ranges = &r.files[0].conversations[0].ranges;
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 2);
        assert!(r.vcs.is_some());
    }

    #[test]
    fn broken_lines_are_skipped() {
        let text = format!("not json at all\n{}\n{{\"truncated\":", FULL_RECORD);
        let records = parse_jsonl(&text);
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn empty_input_yields_no_records() {
        assert!(parse_jsonl("").is_empty());
        assert!(parse_jsonl("\n\n").is_empty());
    }

    #[test]
    fn human_only_record_is_not_ai() {
        let line = r#"{"timestamp":"t","vcs":{"type":"jj","revision":"abcdefgh"},"tool":{"name":"x"},"files":[{"path":"a.rs","conversations":[{"contributor":{"type":"human"},"ranges":[{"start_line":1,"end_line":2}]}]}]}"#;
        let r = parse_one(line).unwrap();
        assert!(!r.has_ai_contribution());
    }

    #[test]
    fn mixed_contributor_counts_as_ai() {
        let line = r#"{"timestamp":"t","files":[{"path":"a.rs","conversations":[{"contributor":{"type":"mixed"},"ranges":[]}]}]}"#;
        let r = parse_one(line).unwrap();
        assert!(r.has_ai_contribution());
    }

    #[test]
    fn pseudo_files_only_record_is_not_ai() {
        let line = r#"{"timestamp":"t","tool":{"name":"claude-code"},"files":[{"path":".shell-history","conversations":[]},{"path":".sessions","conversations":[]}]}"#;
        let r = parse_one(line).unwrap();
        assert!(!r.has_ai_contribution());
    }

    #[test]
    fn range_level_ai_contributor_counts() {
        // conversation says human, but one range is overridden to ai
        let line = r#"{"timestamp":"t","tool":{"name":"x"},"files":[{"path":"a.rs","conversations":[{"contributor":{"type":"human"},"ranges":[{"start_line":1,"end_line":2,"contributor":{"type":"ai"}}]}]}]}"#;
        let r = parse_one(line).unwrap();
        assert!(r.has_ai_contribution());
    }

    #[test]
    fn skip_partial_first_line_drops_to_newline() {
        assert_eq!(skip_partial_first_line("partial\nrest\n"), "rest\n");
        assert_eq!(skip_partial_first_line("no newline"), "");
    }

    #[test]
    fn load_missing_file_returns_none() {
        assert!(load(Path::new("/nonexistent/path/traces.jsonl")).is_none());
    }

    #[test]
    fn load_reads_tempfile() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("traces.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "{}", FULL_RECORD).unwrap();
        writeln!(f, "garbage line").unwrap();

        let result = load(&path).unwrap();
        assert_eq!(result.records.len(), 1);
        assert!(!result.truncated);
    }
}
