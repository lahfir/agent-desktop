use super::*;
use crate::refs_test_support::HomeGuard;
use serde_json::json;
use std::fs;

#[cfg(unix)]
#[test]
fn trace_open_rejects_symlink_paths() {
    let base = std::env::temp_dir().join(format!(
        "agent-desktop-trace-symlink-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let target = base.with_extension("target");
    let link = base.with_extension("link");
    fs::write(&target, b"existing").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let result = open_trace_file(&link);

    assert!(result.is_err());
    let _ = fs::remove_file(&link);
    let _ = fs::remove_file(&target);
}

#[test]
fn trace_write_rejects_files_at_size_cap() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-trace-cap-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let file = fs::File::create(&path).unwrap();
    file.set_len(MAX_TRACE_FILE_BYTES).unwrap();
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut file = open_trace_file(&path).unwrap();

    let err = write_event(&mut file, "event", None, json!({})).unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    let _ = fs::remove_file(path);
}

#[test]
fn segment_configs_in_same_process_share_filename() {
    let dir_a = std::env::temp_dir().join(format!(
        "agent-desktop-seg-a-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dir_b = dir_a.with_extension("b");
    let path_a = segment_path_for_dir(&dir_a);
    let path_b = segment_path_for_dir(&dir_b);
    assert_eq!(path_a.file_name(), path_b.file_name());
}

#[test]
fn explicit_trace_opens_eagerly_at_build() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-eager-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_file(&path);
    let config = TraceConfig::build(Some(path.clone()), None, false).unwrap();
    assert!(
        path.exists(),
        "explicit --trace validates and opens the destination at build (fail-fast)"
    );
    config.emit("event", None, json!({})).unwrap();
    assert!(path.exists());
    let _ = fs::remove_file(path);
}

#[test]
fn segment_lazy_open_creates_trace_dir_on_first_event() {
    let _guard = HomeGuard::new();
    let trace_dir = crate::refs_store::RefStore::for_session(Some("run-42"))
        .unwrap()
        .trace_dir();
    assert!(!trace_dir.exists());
    let config = TraceConfig::build(None, Some(trace_dir.clone()), false).unwrap();
    config.emit("event", Some("run-42"), json!({})).unwrap();
    assert!(trace_dir.is_dir());
    let segment = segment_path_for_dir(&trace_dir);
    assert!(segment.is_file());
    assert!(segment.metadata().unwrap().len() > 0);
}

#[test]
fn write_event_emits_single_atomic_jsonl_line_with_seq() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-atomic-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = TraceConfig::build(Some(path.clone()), None, false).unwrap();
    config.emit("first", None, json!({})).unwrap();
    config.emit("second", None, json!({})).unwrap();
    let body = fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 3);
    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    let third: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(first["event"], "trace.meta");
    assert_eq!(second["event"], "first");
    assert_eq!(third["event"], "second");
    assert!(second["seq"].as_u64().is_some());
    assert!(third["seq"].as_u64().is_some());
    assert!(third["seq"].as_u64().unwrap() > second["seq"].as_u64().unwrap());
    let _ = fs::remove_file(path);
}

#[test]
fn truncated_final_line_leaves_prior_lines_parseable() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-trunc-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = TraceConfig::build(Some(path.clone()), None, false).unwrap();
    config.emit("ok", None, json!({})).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(b"{\"event\":\"partial");
    fs::write(&path, bytes).unwrap();
    let body = fs::read_to_string(&path).unwrap();
    let mut parsed = 0usize;
    for line in body.lines() {
        if serde_json::from_str::<serde_json::Value>(line).is_ok() {
            parsed += 1;
        }
    }
    assert_eq!(parsed, 2);
    let _ = fs::remove_file(path);
}

#[test]
fn strict_missing_trace_path_fails_at_build() {
    let missing = std::env::temp_dir()
        .join("agent-desktop-missing-dir")
        .join("trace.jsonl");
    assert!(TraceConfig::build(Some(missing), None, true).is_err());
}

#[test]
fn best_effort_missing_trace_path_succeeds_silently() {
    let missing = std::env::temp_dir()
        .join("agent-desktop-missing-dir-best-effort")
        .join("trace.jsonl");
    let config = TraceConfig::build(Some(missing), None, false).unwrap();
    assert!(config.emit("event", None, json!({})).is_ok());
}

#[cfg(unix)]
#[test]
fn trace_file_is_private_on_create() {
    use std::os::unix::fs::PermissionsExt;

    let path = std::env::temp_dir().join(format!(
        "agent-desktop-private-trace-{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = TraceConfig::build(Some(path.clone()), None, true).unwrap();
    config.emit("event", None, json!({})).unwrap();

    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let _ = fs::remove_file(path);
}

#[cfg(unix)]
#[test]
fn trace_rejects_loose_existing_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let path = std::env::temp_dir().join(format!(
        "agent-desktop-loose-trace-{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::write(&path, "").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

    let err = TraceConfig::build(Some(path.clone()), None, false).unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    let _ = fs::remove_file(path);
}

#[cfg(unix)]
#[test]
fn segment_open_rejects_symlinked_trace_dir() {
    let base = std::env::temp_dir().join(format!(
        "agent-desktop-symtrace-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let real = base.with_extension("real");
    fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, &base).unwrap();

    let err = super::open_segment_trace_file(&base).unwrap_err();
    assert_eq!(err.code(), "INVALID_ARGS");

    let _ = fs::remove_file(&base);
    let _ = fs::remove_dir_all(&real);
}

#[test]
fn trace_meta_is_first_line_of_new_segment() {
    let trace_dir = std::env::temp_dir().join(format!(
        "agent-desktop-meta-dir-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = TraceConfig::build(None, Some(trace_dir.clone()), false).unwrap();
    config
        .emit(
            "snapshot.saved",
            Some("sess-1"),
            json!({ "snapshot_id": "s1" }),
        )
        .unwrap();
    let segment = segment_path_for_dir(&trace_dir);
    let body = fs::read_to_string(segment).unwrap();
    let first: serde_json::Value = serde_json::from_str(body.lines().next().unwrap()).unwrap();
    assert_eq!(first["event"], "trace.meta");
    assert_eq!(first["schema"], 1);
    assert_eq!(first["session_id"], "sess-1");
    assert!(first["pid"].as_u64().is_some());
    let _ = fs::remove_dir_all(trace_dir);
}

#[test]
fn explicit_trace_file_opens_with_meta_header() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-meta-file-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let config = TraceConfig::build(Some(path.clone()), None, false).unwrap();
    config.emit("event", None, json!({})).unwrap();
    let first: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&path).unwrap().lines().next().unwrap()).unwrap();
    assert_eq!(first["event"], "trace.meta");
    let _ = fs::remove_file(path);
}

#[test]
fn failed_writer_reports_no_sink() {
    let missing = std::env::temp_dir()
        .join("agent-desktop-nodir-hassink")
        .join("trace.jsonl");
    let config = TraceConfig::build(Some(missing), None, false).unwrap();
    assert!(
        !config.has_sink(),
        "a trace writer whose open failed must not report an active sink"
    );
}
