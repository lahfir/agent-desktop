use crate::error::AppError;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub(crate) const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const KNOWN_SCHEMA_MAX: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedSegmentName {
    pub stem: String,
    pub pid: u32,
    pub proc_start_ms: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedEvent {
    pub value: Value,
    pub ts_ms: u64,
    pub position: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SegmentReadStats {
    pub event_count: usize,
    pub skipped_lines: usize,
    pub schema: u32,
    pub schema_warning: Option<String>,
    pub meta_seen: bool,
}

pub(crate) fn parse_segment_filename(name: &str) -> Option<ParsedSegmentName> {
    if !name.ends_with(".jsonl") {
        return None;
    }
    let stem = name.strip_suffix(".jsonl")?;
    if stem.ends_with(".tmp") {
        return None;
    }
    let (pid_str, ts_str) = stem.rsplit_once('-')?;
    if pid_str.is_empty() || ts_str.is_empty() {
        return None;
    }
    if !pid_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !ts_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let pid: u32 = pid_str.parse().ok()?;
    let proc_start_ms: u64 = ts_str.parse().ok()?;
    Some(ParsedSegmentName {
        stem: stem.to_string(),
        pid,
        proc_start_ms,
    })
}

pub(crate) fn open_segment_file(path: &Path) -> Result<std::fs::File, std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
    }
    #[cfg(not(unix))]
    {
        if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "segment path must not be a symlink",
            ));
        }
        std::fs::File::open(path)
    }
}

pub(crate) fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

pub(crate) fn read_segment_events(
    path: &Path,
    writer_pid: u32,
) -> Result<(Vec<ParsedEvent>, SegmentReadStats), AppError> {
    let file = open_segment_file(path).map_err(AppError::from)?;
    let mut reader = BufReader::new(file);
    let mut raw = String::new();
    let mut stats = SegmentReadStats::default();
    let mut events = Vec::new();
    let mut position: u64 = 0;

    loop {
        raw.clear();
        let bytes_read = reader.read_line(&mut raw)?;
        if bytes_read == 0 {
            break;
        }
        position += 1;
        let has_trailing_newline = raw.ends_with('\n');
        let line_body = if has_trailing_newline {
            &raw[..raw.len() - 1]
        } else {
            raw.as_str()
        };

        if line_body.is_empty() {
            continue;
        }

        if line_body.len() > MAX_LINE_BYTES {
            stats.skipped_lines += 1;
            continue;
        }

        if !has_trailing_newline {
            stats.skipped_lines += 1;
            continue;
        }

        let parsed: Value = match serde_json::from_str(line_body) {
            Ok(v) => v,
            Err(_) => {
                stats.skipped_lines += 1;
                continue;
            }
        };

        let Some(obj) = parsed.as_object() else {
            stats.skipped_lines += 1;
            continue;
        };

        if obj.get("event").and_then(Value::as_str) == Some("trace.meta") {
            if !stats.meta_seen {
                stats.meta_seen = true;
                if let Some(schema) = obj.get("schema").and_then(Value::as_u64) {
                    stats.schema = schema as u32;
                    if stats.schema > KNOWN_SCHEMA_MAX {
                        stats.schema_warning = Some(format!(
                            "Segment schema {schema} exceeds reader maximum {KNOWN_SCHEMA_MAX}"
                        ));
                    }
                }
            }
        }

        let ts_ms = obj.get("ts_ms").and_then(Value::as_u64).unwrap_or(0);

        events.push(ParsedEvent {
            value: parsed,
            ts_ms,
            position,
        });
        stats.event_count += 1;
        let _ = writer_pid;
    }

    if !stats.meta_seen {
        stats.schema = 0;
    }

    Ok((events, stats))
}
