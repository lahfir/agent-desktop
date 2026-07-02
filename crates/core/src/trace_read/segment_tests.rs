use super::segment::{MAX_LINE_BYTES, parse_segment_filename, read_segment_events};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("agent-desktop-{prefix}-{nanos}"))
}

fn write_segment(dir: &Path, name: &str, lines: &[&str]) -> std::path::PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    let mut file = fs::File::create(&path).unwrap();
    for line in lines {
        writeln!(file, "{line}").unwrap();
    }
    path
}

#[test]
fn parse_valid_segment_filename() {
    let parsed = parse_segment_filename("4242-1719900000000.jsonl").unwrap();
    assert_eq!(parsed.pid, 4242);
    assert_eq!(parsed.proc_start_ms, 1719900000000);
    assert_eq!(parsed.stem, "4242-1719900000000");
}

#[test]
fn parse_rejects_invalid_filenames() {
    assert!(parse_segment_filename("abc-1.jsonl").is_none());
    assert!(parse_segment_filename("1.jsonl").is_none());
    assert!(parse_segment_filename("notes.txt").is_none());
    assert!(parse_segment_filename("123-9.jsonl.tmp").is_none());
}

#[test]
fn truncated_final_line_is_skipped() {
    let dir = temp_dir("trace-seg-trunc");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("100-1000.jsonl");
    let mut file = fs::File::create(&path).unwrap();
    writeln!(file, r#"{{"event":"click","ts_ms":1000,"seq":1}}"#).unwrap();
    write!(file, r#"{{"event":"click","ts_ms":1001,"seq":2,"trunc"#).unwrap();

    let (_, stats) = read_segment_events(&path, 100).unwrap();
    assert_eq!(stats.skipped_lines, 1);
    assert_eq!(stats.event_count, 1);
}

#[test]
fn corrupt_middle_line_is_skipped() {
    let dir = temp_dir("trace-seg-trunc");
    let path = write_segment(
        &dir,
        "100-1000.jsonl",
        &[
            r#"{"event":"a","ts_ms":1,"seq":1}"#,
            "not json at all",
            r#"{"event":"b","ts_ms":2,"seq":2}"#,
        ],
    );

    let (events, stats) = read_segment_events(&path, 100).unwrap();
    assert_eq!(stats.skipped_lines, 1);
    assert_eq!(events.len(), 2);
}

#[test]
fn non_object_json_line_is_skipped() {
    let dir = temp_dir("trace-seg-trunc");
    let path = write_segment(
        &dir,
        "100-1000.jsonl",
        &[
            "[1,2,3]",
            r#""string""#,
            r#"{"event":"ok","ts_ms":1,"seq":1}"#,
        ],
    );

    let (events, stats) = read_segment_events(&path, 100).unwrap();
    assert_eq!(stats.skipped_lines, 2);
    assert_eq!(events.len(), 1);
}

#[test]
fn oversized_line_is_skipped() {
    let dir = temp_dir("trace-seg-trunc");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("100-1000.jsonl");
    let mut file = fs::File::create(&path).unwrap();
    let big = "x".repeat(MAX_LINE_BYTES + 1);
    writeln!(file, "{{\"event\":\"big\",\"data\":\"{big}\"}}").unwrap();
    writeln!(file, r#"{{"event":"ok","ts_ms":1,"seq":1}}"#).unwrap();

    let (events, stats) = read_segment_events(&path, 100).unwrap();
    assert_eq!(stats.skipped_lines, 1);
    assert_eq!(events.len(), 1);
}

#[test]
fn trace_meta_sets_schema() {
    let dir = temp_dir("trace-seg-trunc");
    let path = write_segment(
        &dir,
        "100-1000.jsonl",
        &[
            r#"{"event":"trace.meta","schema":1,"pid":100,"ts_ms":0,"seq":0}"#,
            r#"{"event":"click","ts_ms":1,"seq":1}"#,
        ],
    );

    let (_, stats) = read_segment_events(&path, 100).unwrap();
    assert_eq!(stats.schema, 1);
}

#[test]
fn absent_meta_reads_as_schema_zero() {
    let dir = temp_dir("trace-seg-trunc");
    let path = write_segment(
        &dir,
        "100-1000.jsonl",
        &[r#"{"event":"click","ts_ms":1,"seq":1}"#],
    );

    let (_, stats) = read_segment_events(&path, 100).unwrap();
    assert_eq!(stats.schema, 0);
    assert!(stats.schema_warning.is_none());
}

#[test]
fn schema_two_produces_warning() {
    let dir = temp_dir("trace-seg-trunc");
    let path = write_segment(
        &dir,
        "100-1000.jsonl",
        &[
            r#"{"event":"trace.meta","schema":2,"pid":100,"ts_ms":0,"seq":0}"#,
            r#"{"event":"click","ts_ms":1,"seq":1}"#,
        ],
    );

    let (_, stats) = read_segment_events(&path, 100).unwrap();
    assert_eq!(stats.schema, 2);
    assert!(stats.schema_warning.is_some());
}

#[test]
fn multiple_meta_lines_only_first_counts_for_schema() {
    let dir = temp_dir("trace-seg-trunc");
    let path = write_segment(
        &dir,
        "100-1000.jsonl",
        &[
            r#"{"event":"trace.meta","schema":1,"pid":100,"ts_ms":0,"seq":0}"#,
            r#"{"event":"trace.meta","schema":2,"pid":100,"ts_ms":1,"seq":1}"#,
            r#"{"event":"click","ts_ms":2,"seq":2}"#,
        ],
    );

    let (events, stats) = read_segment_events(&path, 100).unwrap();
    assert_eq!(stats.schema, 1);
    assert_eq!(events.len(), 3);
}

#[cfg(unix)]
#[test]
fn symlinked_segment_is_detected() {
    let dir = temp_dir("trace-seg-symlink");
    fs::create_dir_all(&dir).unwrap();
    let target = dir.join("100-1000.jsonl");
    fs::write(&target, b"{\"event\":\"a\",\"ts_ms\":1,\"seq\":1}\n").unwrap();
    let link = dir.join("200-2000.jsonl");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(super::segment::is_symlink(&link));
}
