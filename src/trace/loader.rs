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
    TraceRelated, TraceVcs, TraceVcsType,
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
    // Read as BYTES: a tail-read may start mid-way through a multi-byte UTF-8
    // character, which would make read_to_string fail and silently disable
    // the whole feature even though valid records follow. The partial first
    // line is dropped byte-wise below, and lossy decoding keeps any other
    // stray invalid bytes from rejecting the readable remainder (P1).
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;

    let body_bytes = if truncated {
        skip_partial_first_line(&bytes)
    } else {
        bytes.as_slice()
    };
    let text = String::from_utf8_lossy(body_bytes);

    Some(LoadResult {
        records: parse_jsonl(&text),
        truncated,
    })
}

/// Drop everything up to and including the first newline (a tail-read starts
/// mid-line — possibly mid-character — so this must operate on bytes).
fn skip_partial_first_line(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|&b| b == b'\n') {
        Some(pos) => &bytes[pos + 1..],
        None => &[],
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
        related: v
            .get("related")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(parse_related).collect())
            .unwrap_or_default(),
    }
}

/// Parse a `related[]` entry. Both `type` and `url` are required strings —
/// a malformed entry is dropped (not the whole record).
fn parse_related(v: &Value) -> Option<TraceRelated> {
    Some(TraceRelated {
        rel_type: str_field(v, "type")?,
        url: str_field(v, "url")?,
    })
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
    fn parses_related_resources() {
        let line = r#"{"timestamp":"t","vcs":{"type":"jj","revision":"xqnktzmlworukplnyrropmtzylsuxxlv"},"tool":{"name":"x"},"files":[{"path":"a.rs","conversations":[{"url":"u1","contributor":{"type":"ai"},"ranges":[{"start_line":1,"end_line":2}],"related":[{"type":"pull-request","url":"https://gh/pr/1"},{"type":"prompt","url":"u3"}]}]}]}"#;
        let r = parse_one(line).unwrap();
        let conv = &r.files[0].conversations[0];
        assert_eq!(conv.related.len(), 2);
        assert_eq!(conv.related[0].rel_type, "pull-request");
        assert_eq!(conv.related[0].url, "https://gh/pr/1");
        // all_urls() flattens conversation.url + related[]
        let urls = r.all_urls();
        assert_eq!(
            urls,
            vec![
                ("conversation".to_string(), "u1".to_string()),
                ("pull-request".to_string(), "https://gh/pr/1".to_string()),
                ("prompt".to_string(), "u3".to_string()),
            ]
        );
    }

    #[test]
    fn related_malformed_entries_are_dropped_not_record() {
        // one related entry missing url, one missing type → both dropped, the
        // record and its valid pieces survive (P1)
        let line = r#"{"timestamp":"t","vcs":{"type":"jj","revision":"xqnktzmlworukplnyrropmtzylsuxxlv"},"tool":{"name":"x"},"files":[{"path":"a.rs","conversations":[{"contributor":{"type":"ai"},"ranges":[],"related":[{"type":"x"},{"url":"y"},{"type":"ok","url":"z"}]}]}]}"#;
        let r = parse_one(line).unwrap();
        let conv = &r.files[0].conversations[0];
        assert_eq!(conv.related.len(), 1);
        assert_eq!(conv.related[0].rel_type, "ok");
        assert_eq!(conv.related[0].url, "z");
    }

    #[test]
    fn all_urls_empty_when_no_urls() {
        let line = r#"{"timestamp":"t","tool":{"name":"x"},"files":[{"path":"a.rs","conversations":[{"contributor":{"type":"ai"},"ranges":[]}]}]}"#;
        let r = parse_one(line).unwrap();
        assert!(r.all_urls().is_empty());
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
        assert_eq!(skip_partial_first_line(b"partial\nrest\n"), b"rest\n");
        assert_eq!(skip_partial_first_line(b"no newline"), b"");
        // Mid-multibyte-character start must not panic (byte-wise scan)
        let e_acute = "é".as_bytes(); // [0xC3, 0xA9]
        let input = [&e_acute[1..], b"\nrest"].concat();
        assert_eq!(skip_partial_first_line(&input), b"rest");
    }

    #[test]
    fn truncated_tail_starting_mid_multibyte_char_still_loads() {
        use std::io::Write;

        // Build a file of MAX_TRACE_BYTES + 1 bytes where the tail-read seek
        // position (len - MAX) lands on byte 1 — the middle of the leading
        // 'é' (2 bytes). The old read_to_string-based loader returned None
        // here, disabling the feature despite a valid record in the tail.
        let tail = format!("\n{}\n", FULL_RECORD);
        let max = MAX_TRACE_BYTES as usize;
        let head_len = max - tail.len() + 1;
        let mut head = "é".repeat(head_len / 2);
        if head_len % 2 == 1 {
            head.push('a');
        }
        assert_eq!(head.len(), head_len);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("traces.jsonl");
        let mut f = File::create(&path).unwrap();
        f.write_all(head.as_bytes()).unwrap();
        f.write_all(tail.as_bytes()).unwrap();
        drop(f);

        let result = load(&path).expect("mid-char tail must not disable loading");
        assert!(result.truncated);
        assert_eq!(result.records.len(), 1);
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
