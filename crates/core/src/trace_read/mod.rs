mod html;
mod merge;
mod segment;

pub use html::{ExportOptions, ExportStats, TRACE_EXPORT_DEFAULT_LIMIT, export_html};

use crate::error::AppError;
use merge::{
    annotate_provenance, apply_tail_limit, detect_unpaired_commands, filter_by_event_prefix,
    merge_segments,
};
use segment::{SegmentReadStats, parse_segment_filename, read_segment_events};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct ReadOptions {
    pub limit: usize,
    pub event_prefix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceWarningKind {
    ForeignFile,
    UnreadableSegment,
    SymlinkedSegment,
    SchemaUnknown,
    UnpairedCommand,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceWarning {
    pub kind: TraceWarningKind,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SegmentInfo {
    pub segment: String,
    pub pid: u32,
    pub schema: u32,
    pub event_count: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub skipped_lines: usize,
}

fn is_zero(v: &usize) -> bool {
    *v == 0
}

#[derive(Debug, Clone)]
pub struct MergedTrace {
    pub events: Vec<Value>,
    pub segments: Vec<SegmentInfo>,
    pub segments_truncated: bool,
    pub warnings: Vec<TraceWarning>,
    pub warnings_truncated: bool,
    pub total_events: usize,
    pub matched_events: Option<usize>,
    pub returned_events: usize,
    pub truncated: bool,
}

const METADATA_LIST_CAP: usize = 500;

pub fn read_merged(trace_dir: &Path, options: &ReadOptions) -> Result<MergedTrace, AppError> {
    if !trace_dir.is_dir() {
        return Err(AppError::invalid_input_with_suggestion(
            "Trace directory does not exist",
            "Run `session start` with tracing enabled, or pass `--session <id>`.",
        ));
    }

    let mut warnings = Vec::new();
    let mut segment_infos = Vec::new();
    let mut merge_sources = Vec::new();

    for entry in std::fs::read_dir(trace_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };

        if name.ends_with(".jsonl.tmp") {
            continue;
        }

        let Some(parsed_name) = parse_segment_filename(name) else {
            if !name.starts_with('.') {
                warnings.push(TraceWarning {
                    kind: TraceWarningKind::ForeignFile,
                    message: format!("Ignoring foreign file in trace directory: {name}"),
                });
            }
            continue;
        };
        let path = entry.path();

        if crate::refs::is_symlink(&path) {
            warnings.push(TraceWarning {
                kind: TraceWarningKind::SymlinkedSegment,
                message: format!("Skipping symlinked segment: {name}"),
            });
            continue;
        }

        match read_segment_events(&path) {
            Ok((events, stats)) => {
                if let Some(ref msg) = stats.schema_warning {
                    warnings.push(TraceWarning {
                        kind: TraceWarningKind::SchemaUnknown,
                        message: msg.clone(),
                    });
                }
                segment_infos.push(segment_info_from_stats(&parsed_name, &stats));
                merge_sources.push((events, parsed_name.pid, parsed_name.stem));
            }
            Err(err) => {
                warnings.push(TraceWarning {
                    kind: TraceWarningKind::UnreadableSegment,
                    message: format!("Skipping unreadable segment {name}: {err}"),
                });
            }
        }
    }

    segment_infos.sort_by(|a, b| a.segment.cmp(&b.segment));
    merge_sources.sort_by(|a, b| a.2.cmp(&b.2));

    let merged = merge_segments(merge_sources);
    let all_events: Vec<Value> = merged.iter().map(annotate_provenance).collect();
    let total_events = all_events.len();

    for msg in detect_unpaired_commands(&all_events) {
        warnings.push(TraceWarning {
            kind: TraceWarningKind::UnpairedCommand,
            message: msg,
        });
    }

    let filtered_events = filter_by_event_prefix(all_events, options.event_prefix.as_deref());
    let matched_events = if options.event_prefix.is_some() {
        Some(filtered_events.len())
    } else {
        None
    };

    let (returned_events, truncated) = apply_tail_limit(filtered_events, options.limit);

    let (segments, segments_truncated) = cap_list(segment_infos, METADATA_LIST_CAP);
    let (warnings, warnings_truncated) = cap_list(warnings, METADATA_LIST_CAP);

    Ok(MergedTrace {
        returned_events: returned_events.len(),
        events: returned_events,
        segments,
        segments_truncated,
        warnings,
        warnings_truncated,
        total_events,
        matched_events,
        truncated,
    })
}

fn cap_list<T>(mut items: Vec<T>, cap: usize) -> (Vec<T>, bool) {
    if items.len() <= cap {
        return (items, false);
    }
    items.truncate(cap);
    (items, true)
}

fn segment_info_from_stats(
    parsed: &segment::ParsedSegmentName,
    stats: &SegmentReadStats,
) -> SegmentInfo {
    SegmentInfo {
        segment: parsed.stem.clone(),
        pid: parsed.pid,
        schema: stats.schema,
        event_count: stats.event_count,
        skipped_lines: stats.skipped_lines,
    }
}

#[cfg(test)]
#[path = "segment_tests.rs"]
mod segment_tests;

#[cfg(test)]
#[path = "merge_tests.rs"]
mod merge_tests;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
