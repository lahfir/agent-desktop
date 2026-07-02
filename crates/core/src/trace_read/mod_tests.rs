use super::{ReadOptions, TraceWarningKind, read_merged};
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

fn write_segment(dir: &Path, name: &str, lines: &[&str]) {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    let mut file = fs::File::create(&path).unwrap();
    for line in lines {
        writeln!(file, "{line}").unwrap();
    }
}

#[test]
fn empty_trace_directory_yields_empty_timeline() {
    let dir = temp_dir("trace-mod-empty");
    fs::create_dir(&dir).unwrap();
    let result = read_merged(&dir, &ReadOptions::default()).unwrap();
    assert!(result.events.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn missing_trace_directory_returns_error() {
    let dir = temp_dir("trace-mod-missing");
    let result = read_merged(&dir, &ReadOptions::default());
    assert!(result.is_err());
}

#[test]
fn foreign_file_produces_warning() {
    let dir = temp_dir("trace-mod-foreign");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("notes.txt"), b"hello").unwrap();
    let result = read_merged(&dir, &ReadOptions::default()).unwrap();
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == TraceWarningKind::ForeignFile)
    );
}

#[test]
fn tmp_file_is_silently_ignored() {
    let dir = temp_dir("trace-mod-tmp");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("100-1.jsonl.tmp"), b"data").unwrap();
    let result = read_merged(&dir, &ReadOptions::default()).unwrap();
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.kind == TraceWarningKind::ForeignFile)
    );
}

#[test]
fn two_segments_merge_with_provenance() {
    let dir = temp_dir("trace-mod-merge");
    write_segment(
        &dir,
        "100-1000.jsonl",
        &[r#"{"event":"a","ts_ms":100,"seq":1,"pid":555}"#],
    );
    write_segment(
        &dir,
        "200-2000.jsonl",
        &[r#"{"event":"b","ts_ms":200,"seq":1}"#],
    );

    let result = read_merged(&dir, &ReadOptions::default()).unwrap();
    assert_eq!(result.events.len(), 2);
    assert_eq!(result.events[0]["writer_pid"], 100);
    assert_eq!(result.events[0]["pid"], 555);
    assert_eq!(result.events[1]["writer_pid"], 200);
    assert_eq!(result.segments.len(), 2);
}

#[test]
fn schema_unknown_warning_for_future_schema() {
    let dir = temp_dir("trace-mod-schema");
    write_segment(
        &dir,
        "100-1000.jsonl",
        &[
            r#"{"event":"trace.meta","schema":2,"pid":100,"ts_ms":0,"seq":0}"#,
            r#"{"event":"future","ts_ms":1,"seq":1,"extra":true}"#,
        ],
    );

    let result = read_merged(&dir, &ReadOptions::default()).unwrap();
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == TraceWarningKind::SchemaUnknown)
    );
    assert_eq!(result.events[1]["extra"], true);
}

#[test]
fn tail_limit_marks_truncated_and_unpaired() {
    let dir = temp_dir("trace-mod-tail");
    write_segment(
        &dir,
        "100-1000.jsonl",
        &[
            r#"{"event":"command.start","command":"click","ts_ms":1,"seq":1}"#,
            r#"{"event":"command.end","command":"click","ok":true,"ts_ms":2,"seq":2}"#,
            r#"{"event":"command.start","command":"type","ts_ms":3,"seq":3}"#,
            r#"{"event":"command.end","command":"type","ok":true,"ts_ms":4,"seq":4}"#,
        ],
    );

    let result = read_merged(
        &dir,
        &ReadOptions {
            limit: 3,
            event_prefix: None,
        },
    )
    .unwrap();
    assert!(result.truncated);
    assert_eq!(result.returned_events, 3);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == TraceWarningKind::UnpairedCommand)
    );
}

#[cfg(unix)]
#[test]
fn unreadable_segment_is_skipped_with_warning() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("trace-mod-unread");
    fs::create_dir(&dir).unwrap();
    let seg = dir.join("100-1000.jsonl");
    fs::write(&seg, b"{\"event\":\"a\",\"ts_ms\":1,\"seq\":1}\n").unwrap();
    fs::set_permissions(&seg, fs::Permissions::from_mode(0o000)).unwrap();
    write_segment(
        &dir,
        "200-2000.jsonl",
        &[r#"{"event":"b","ts_ms":2,"seq":1}"#],
    );

    let result = read_merged(&dir, &ReadOptions::default()).unwrap();
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == TraceWarningKind::UnreadableSegment)
    );
    assert_eq!(result.events.len(), 1);

    fs::set_permissions(&seg, fs::Permissions::from_mode(0o644)).unwrap();
}

#[cfg(unix)]
#[test]
fn symlinked_segment_is_skipped_with_warning() {
    let dir = temp_dir("trace-mod-symlink");
    fs::create_dir(&dir).unwrap();
    let target = dir.join("100-1000.jsonl");
    fs::write(&target, b"{\"event\":\"a\",\"ts_ms\":1,\"seq\":1}\n").unwrap();
    let link = dir.join("200-2000.jsonl");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    write_segment(
        &dir,
        "300-3000.jsonl",
        &[r#"{"event":"b","ts_ms":2,"seq":1}"#],
    );

    let result = read_merged(&dir, &ReadOptions::default()).unwrap();
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == TraceWarningKind::SymlinkedSegment)
    );
    assert_eq!(result.events.len(), 2);
}
