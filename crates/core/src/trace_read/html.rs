use super::{ReadOptions, TraceWarning, read_merged};
use crate::error::AppError;
use crate::trace_artifacts::read_screenshot_for_embed;
use base64::Engine;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

const VIEWER_HTML: &str = include_str!("viewer.html");
const VIEWER_CSS: &str = include_str!("viewer.css");
const VIEWER_JS: &str = include_str!("viewer.js");

pub const TRACE_EXPORT_DEFAULT_LIMIT: usize = 5000;
const MAX_EMBED_SCREENSHOT_BYTES: u64 = 100 * 1024 * 1024;
const MAX_JSON_BYTES: u64 = 200 * 1024 * 1024;

#[cfg(test)]
static TEST_MAX_JSON_BYTES: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
pub(crate) fn set_test_max_json_bytes(limit: u64) {
    TEST_MAX_JSON_BYTES.store(limit, Ordering::Relaxed);
}

#[cfg(test)]
pub(crate) fn clear_test_max_json_bytes() {
    TEST_MAX_JSON_BYTES.store(0, Ordering::Relaxed);
}

fn max_json_bytes() -> u64 {
    #[cfg(test)]
    {
        let limit = TEST_MAX_JSON_BYTES.load(Ordering::Relaxed);
        if limit > 0 {
            return limit;
        }
    }
    MAX_JSON_BYTES
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub limit: usize,
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportStats {
    pub path: String,
    pub event_count: usize,
    pub screenshots_embedded: usize,
    pub screenshots_skipped: usize,
    pub bytes: usize,
}

pub fn export_html(
    trace_dir: &Path,
    session_id: &str,
    options: &ExportOptions,
) -> Result<(String, ExportStats), AppError> {
    let merged = read_merged(
        trace_dir,
        &ReadOptions {
            limit: options.limit,
            event_prefix: None,
        },
    )?;

    let (screenshots, embedded, skipped) = embed_screenshots(trace_dir, &merged.events)?;
    let warnings: Vec<Value> = merged.warnings.iter().map(serialize_warning).collect();

    let island = json!({
        "session_id": session_id,
        "total_events": merged.total_events,
        "returned_events": merged.returned_events,
        "truncated": merged.truncated,
        "warnings": warnings,
        "screenshots": screenshots,
        "screenshots_embedded": embedded,
        "screenshots_skipped": skipped,
        "events": merged.events,
    });

    let island_json = serde_json::to_string(&island)?;
    if island_json.len() as u64 > max_json_bytes() {
        return Err(AppError::invalid_input_with_suggestion(
            "Trace export exceeds the maximum embedded JSON size",
            "Re-run with a smaller --limit to embed fewer events.",
        ));
    }

    let escaped = escape_for_json_island(&island_json);
    let html = VIEWER_HTML
        .replace("{{CSS}}", VIEWER_CSS)
        .replace("{{JS}}", VIEWER_JS)
        .replace("{{DATA}}", &escaped);

    if html.len() as u64 > MAX_JSON_BYTES + (512 * 1024) {
        return Err(AppError::invalid_input_with_suggestion(
            "Trace export exceeds the maximum output size",
            "Re-run with a smaller --limit to embed fewer events.",
        ));
    }

    let path = options
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("trace-{session_id}.html")));

    write_export_file(&path, html.as_bytes())?;
    let bytes = html.len();

    Ok((
        html,
        ExportStats {
            path: path.to_string_lossy().into_owned(),
            event_count: merged.returned_events,
            screenshots_embedded: embedded,
            screenshots_skipped: skipped,
            bytes,
        },
    ))
}

fn serialize_warning(warning: &TraceWarning) -> Value {
    json!({
        "kind": warning_kind(warning),
        "message": warning.message,
    })
}

fn warning_kind(warning: &TraceWarning) -> &'static str {
    use super::TraceWarningKind;
    match warning.kind {
        TraceWarningKind::ForeignFile => "foreign_file",
        TraceWarningKind::UnreadableSegment => "unreadable_segment",
        TraceWarningKind::SymlinkedSegment => "symlinked_segment",
        TraceWarningKind::SchemaUnknown => "schema_unknown",
        TraceWarningKind::UnpairedCommand => "unpaired_command",
    }
}

fn embed_screenshots(
    trace_dir: &Path,
    events: &[Value],
) -> Result<(Value, usize, usize), AppError> {
    let mut paths = Vec::new();
    for event in events {
        if event.get("event").and_then(Value::as_str) != Some("action.artifacts") {
            continue;
        }
        for key in ["screenshot_pre", "screenshot_post"] {
            if let Some(path) = event.get(key).and_then(Value::as_str) {
                if !paths.contains(&path.to_string()) {
                    paths.push(path.to_string());
                }
            }
        }
    }

    let mut map = serde_json::Map::new();
    let mut embedded = 0usize;
    let mut skipped = 0usize;
    let mut used_bytes = 0u64;

    for rel in paths {
        let Some(bytes) = read_screenshot_for_embed(trace_dir, &rel) else {
            skipped += 1;
            continue;
        };
        let next = used_bytes.saturating_add(bytes.len() as u64);
        if next > MAX_EMBED_SCREENSHOT_BYTES {
            skipped += 1;
            continue;
        }
        used_bytes = next;
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        map.insert(rel, json!(format!("data:image/png;base64,{encoded}")));
        embedded += 1;
    }

    Ok((Value::Object(map), embedded, skipped))
}

pub fn escape_for_json_island(json: &str) -> String {
    json.chars()
        .map(|ch| match ch {
            '<' => "\\u003c".to_string(),
            '>' => "\\u003e".to_string(),
            '&' => "\\u0026".to_string(),
            '\u{2028}' => "\\u2028".to_string(),
            '\u{2029}' => "\\u2029".to_string(),
            other => other.to_string(),
        })
        .collect()
}

fn write_export_file(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    if path.is_symlink() {
        return Err(AppError::invalid_input_with_suggestion(
            "Refusing to write trace export through a symlink",
            "Choose a different --out path.",
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, bytes).map_err(AppError::from)
}

#[cfg(test)]
#[path = "html_tests.rs"]
mod tests;
