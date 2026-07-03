use super::ActionabilityStatus;
use crate::node::Rect;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ActionabilityCheck {
    pub name: &'static str,
    pub status: ActionabilityStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occluder: Option<Occluder>,
}

/// The element a hit test actually landed on when it failed to reach the
/// intended target. `name` carries the occluder's accessible name under a
/// `name`-keyed field so `sanitize_trace_value` redacts it automatically;
/// `role` is a bounded AX vocabulary token, safe to surface in free text.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Occluder {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,
}
