use super::segment::ParsedEvent;

#[derive(Debug, Clone)]
pub(crate) struct MergeItem {
    pub event: ParsedEvent,
    pub writer_pid: u32,
    pub segment: String,
}

struct StreamState {
    events: Vec<ParsedEvent>,
    index: usize,
    writer_pid: u32,
    segment: String,
}

pub(crate) fn merge_segments(sources: Vec<(Vec<ParsedEvent>, u32, String)>) -> Vec<MergeItem> {
    let mut streams: Vec<StreamState> = sources
        .into_iter()
        .map(|(events, writer_pid, segment)| StreamState {
            events,
            index: 0,
            writer_pid,
            segment,
        })
        .collect();

    let mut merged = Vec::new();
    loop {
        let mut best: Option<usize> = None;
        let mut best_key: Option<(u64, u32, u64)> = None;

        for (i, stream) in streams.iter().enumerate() {
            if stream.index >= stream.events.len() {
                continue;
            }
            let event = &stream.events[stream.index];
            let key = (event.ts_ms, stream.writer_pid, event.position);
            if best_key.is_none_or(|current| key < current) {
                best_key = Some(key);
                best = Some(i);
            }
        }

        let Some(i) = best else {
            break;
        };
        let stream = &streams[i];
        let event = stream.events[stream.index].clone();
        merged.push(MergeItem {
            event,
            writer_pid: stream.writer_pid,
            segment: stream.segment.clone(),
        });
        streams[i].index += 1;
    }
    merged
}

use serde_json::{Map, Value, json};

pub(crate) fn annotate_provenance(item: &MergeItem) -> Value {
    let mut value = item.event.value.clone();
    if let Value::Object(ref mut map) = value {
        map.entry("writer_pid".to_string())
            .or_insert(json!(item.writer_pid));
        map.entry("segment".to_string())
            .or_insert(json!(item.segment));
    }
    value
}

pub(crate) fn filter_by_event_prefix(events: &[Value], prefix: Option<&str>) -> Vec<Value> {
    let Some(prefix) = prefix.filter(|p| !p.is_empty()) else {
        return events.to_vec();
    };
    events
        .iter()
        .filter(|event| {
            event
                .get("event")
                .and_then(Value::as_str)
                .is_some_and(|name| name.starts_with(prefix))
        })
        .cloned()
        .collect()
}

pub(crate) fn apply_tail_limit(events: Vec<Value>, limit: usize) -> (Vec<Value>, bool) {
    if limit == 0 || events.len() <= limit {
        return (events, false);
    }
    let start = events.len() - limit;
    (events[start..].to_vec(), true)
}

pub(crate) fn detect_unpaired_commands(events: &[Value]) -> Vec<String> {
    let mut open: Map<String, Value> = Map::new();
    let mut warnings = Vec::new();

    for event in events {
        let Some(name) = event.get("event").and_then(Value::as_str) else {
            continue;
        };
        let segment = event
            .get("segment")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let writer_pid = event.get("writer_pid").and_then(Value::as_u64).unwrap_or(0);
        let command = event
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let key = format!("{segment}:{writer_pid}:{command}");

        if name == "command.start" {
            open.insert(
                key.clone(),
                json!({ "command": command, "segment": segment, "writer_pid": writer_pid }),
            );
        } else if name == "command.end" {
            if open.remove(&key).is_none() {
                warnings.push(format!(
                    "Unpaired command.end for '{command}' in segment {segment}"
                ));
            }
        }
    }

    for (_, info) in open {
        let command = info
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        warnings.push(format!(
            "Unpaired command.start for '{command}' in segment {}",
            info.get("segment").and_then(Value::as_str).unwrap_or("")
        ));
    }
    warnings
}
