use crate::node::AccessibilityNode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DiffEntry {
    Added(AddedEntry),
    Removed(RemovedEntry),
    Modified(ModifiedEntry),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddedEntry {
    pub path: String,
    pub node: NodeSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedEntry {
    pub path: String,
    pub node: NodeSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedEntry {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: String,
    pub from: serde_json::Value,
    pub to: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSummary {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
}

pub fn diff_trees(old: &AccessibilityNode, new: &AccessibilityNode) -> (Vec<DiffEntry>, DiffStats) {
    let mut entries = Vec::new();
    let mut stats = DiffStats {
        added: 0,
        removed: 0,
        modified: 0,
        unchanged: 0,
    };
    let root_path = format_node_key(old, 0);
    walk(old, new, &root_path, &mut entries, &mut stats);
    (entries, stats)
}

fn walk(
    old: &AccessibilityNode,
    new: &AccessibilityNode,
    path: &str,
    entries: &mut Vec<DiffEntry>,
    stats: &mut DiffStats,
) {
    let changes = compare_fields(old, new);
    if changes.is_empty() {
        stats.unchanged += 1;
    } else {
        stats.modified += 1;
        entries.push(DiffEntry::Modified(ModifiedEntry {
            path: path.to_string(),
            ref_id: new.ref_id.clone(),
            changes,
        }));
    }

    let (paired, old_unmatched, new_unmatched) = match_children(&old.children, &new.children);

    for (old_child, new_child) in paired {
        let child_path = build_child_path(path, &new_child, 0);
        walk(old_child, new_child, &child_path, entries, stats);
    }

    for (idx, node) in old_unmatched {
        let child_path = build_child_path(path, node, idx);
        collect_removed(node, &child_path, entries, stats);
    }

    for (idx, node) in new_unmatched {
        let child_path = build_child_path(path, node, idx);
        collect_added(node, &child_path, entries, stats);
    }
}

fn match_children<'a>(
    old_children: &'a [AccessibilityNode],
    new_children: &'a [AccessibilityNode],
) -> (
    Vec<(&'a AccessibilityNode, &'a AccessibilityNode)>,
    Vec<(usize, &'a AccessibilityNode)>,
    Vec<(usize, &'a AccessibilityNode)>,
) {
    let mut old_used = vec![false; old_children.len()];
    let mut new_used = vec![false; new_children.len()];
    let mut pairs = Vec::new();

    for (ni, new_node) in new_children.iter().enumerate() {
        let mut best: Option<usize> = None;
        let new_key = node_identity_key(new_node);

        for (oi, old_node) in old_children.iter().enumerate() {
            if old_used[oi] {
                continue;
            }
            if node_identity_key(old_node) == new_key {
                best = Some(oi);
                break;
            }
        }

        if let Some(oi) = best {
            old_used[oi] = true;
            new_used[ni] = true;
            pairs.push((&old_children[oi], new_node));
        }
    }

    let old_unmatched: Vec<(usize, &AccessibilityNode)> = old_children
        .iter()
        .enumerate()
        .filter(|(i, _)| !old_used[*i])
        .collect();

    let new_unmatched: Vec<(usize, &AccessibilityNode)> = new_children
        .iter()
        .enumerate()
        .filter(|(i, _)| !new_used[*i])
        .collect();

    (pairs, old_unmatched, new_unmatched)
}

fn node_identity_key(node: &AccessibilityNode) -> String {
    match &node.name {
        Some(name) => format!("{}:{}", node.role, name),
        None => node.role.clone(),
    }
}

fn compare_fields(old: &AccessibilityNode, new: &AccessibilityNode) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    if old.value != new.value {
        changes.push(FieldChange {
            field: "value".into(),
            from: to_json_val(&old.value),
            to: to_json_val(&new.value),
        });
    }

    if old.states != new.states {
        changes.push(FieldChange {
            field: "states".into(),
            from: serde_json::to_value(&old.states).unwrap_or_default(),
            to: serde_json::to_value(&new.states).unwrap_or_default(),
        });
    }

    if old.description != new.description {
        changes.push(FieldChange {
            field: "description".into(),
            from: to_json_val(&old.description),
            to: to_json_val(&new.description),
        });
    }

    changes
}

fn collect_added(
    node: &AccessibilityNode,
    path: &str,
    entries: &mut Vec<DiffEntry>,
    stats: &mut DiffStats,
) {
    stats.added += 1;
    entries.push(DiffEntry::Added(AddedEntry {
        path: path.to_string(),
        node: summarize(node),
    }));
    let mut key_counts: HashMap<String, usize> = HashMap::new();
    for child in &node.children {
        let key = node_identity_key(child);
        let idx = *key_counts.get(&key).unwrap_or(&0);
        key_counts.insert(key, idx + 1);
        let child_path = build_child_path(path, child, idx);
        collect_added(child, &child_path, entries, stats);
    }
}

fn collect_removed(
    node: &AccessibilityNode,
    path: &str,
    entries: &mut Vec<DiffEntry>,
    stats: &mut DiffStats,
) {
    stats.removed += 1;
    entries.push(DiffEntry::Removed(RemovedEntry {
        path: path.to_string(),
        node: summarize(node),
    }));
    let mut key_counts: HashMap<String, usize> = HashMap::new();
    for child in &node.children {
        let key = node_identity_key(child);
        let idx = *key_counts.get(&key).unwrap_or(&0);
        key_counts.insert(key, idx + 1);
        let child_path = build_child_path(path, child, idx);
        collect_removed(child, &child_path, entries, stats);
    }
}

fn build_child_path(parent: &str, node: &AccessibilityNode, sibling_idx: usize) -> String {
    let segment = format_node_key(node, sibling_idx);
    format!("{} > {}", parent, segment)
}

fn format_node_key(node: &AccessibilityNode, sibling_idx: usize) -> String {
    match &node.name {
        Some(name) if !name.is_empty() => {
            let escaped = name.replace('"', "\\\"");
            if sibling_idx == 0 {
                format!("{}[\"{}\"]{}", node.role, escaped, "")
            } else {
                format!("{}[\"{}\"][{}]", node.role, escaped, sibling_idx)
            }
        }
        _ => {
            if sibling_idx == 0 {
                node.role.clone()
            } else {
                format!("{}[{}]", node.role, sibling_idx)
            }
        }
    }
}

fn summarize(node: &AccessibilityNode) -> NodeSummary {
    NodeSummary {
        role: node.role.clone(),
        name: node.name.clone(),
        value: node.value.clone(),
        ref_id: node.ref_id.clone(),
    }
}

fn to_json_val(opt: &Option<String>) -> serde_json::Value {
    match opt {
        Some(s) => serde_json::Value::String(s.clone()),
        None => serde_json::Value::Null,
    }
}

pub fn format_text_diff(entries: &[DiffEntry], stats: &DiffStats) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} change(s): +{} added  -{} removed  ~{} modified  ={} unchanged\n",
        stats.added + stats.removed + stats.modified,
        stats.added,
        stats.removed,
        stats.modified,
        stats.unchanged,
    ));
    for entry in entries {
        match entry {
            DiffEntry::Added(e) => {
                out.push_str(&format!("\x1b[32m+ {} ({})\x1b[0m\n", e.path, e.node.role));
                if let Some(v) = &e.node.value {
                    out.push_str(&format!("\x1b[32m    value: {:?}\x1b[0m\n", v));
                }
            }
            DiffEntry::Removed(e) => {
                out.push_str(&format!("\x1b[31m- {} ({})\x1b[0m\n", e.path, e.node.role));
            }
            DiffEntry::Modified(e) => {
                let ref_label = e
                    .ref_id
                    .as_deref()
                    .map(|r| format!(" {}", r))
                    .unwrap_or_default();
                out.push_str(&format!("\x1b[33m~ {}{}\x1b[0m\n", e.path, ref_label));
                for c in &e.changes {
                    out.push_str(&format!(
                        "\x1b[33m    {}: {} → {}\x1b[0m\n",
                        c.field, c.from, c.to
                    ));
                }
            }
        }
    }
    out
}
