use super::ActionabilityStatus;
use crate::error::ErrorCode;
use crate::node::Rect;
use serde::Serialize;

/// One actionability gate result. `check` is the gate's stable identifier
/// (`visible`, `stable`, `enabled`, `supported_action`, `policy`, `editable`,
/// `receives_events`) — a bounded vocabulary token, deliberately NOT keyed
/// `name` so `sanitize_trace_value` leaves it readable in traces (unlike
/// `Occluder.name`, which is a real element name and must stay redacted).
/// `terminal_code`, set only on a failing check whose failure is permanent
/// (waiting cannot heal it, e.g. an unsupported action or a policy denial),
/// carries the error code the caller should surface; it is not serialized —
/// its effect is that the auto-wait poll loop fails fast instead of retrying
/// to the deadline. Transient failures (offscreen, unstable, disabled,
/// occluded) leave it `None` and remain retryable.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ActionabilityCheck {
    pub check: &'static str,
    pub status: ActionabilityStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occluder: Option<Occluder>,
    #[serde(skip)]
    pub terminal_code: Option<ErrorCode>,
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
