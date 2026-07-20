use serde_json::{Value, json};

use crate::{ActionResult, AdapterError, actionability::StabilityExpectation};

#[derive(Default)]
pub(crate) struct RefActionPollState {
    pub(crate) last_report: Option<Value>,
    pub(crate) saw_ambiguity: bool,
    pub(crate) expected_bounds_hash: Option<u64>,
    pub(crate) resolve_attempts: u64,
    pub(crate) preflight_attempts: u64,
}

impl RefActionPollState {
    pub(crate) fn stability(&self) -> StabilityExpectation {
        StabilityExpectation::strict_hash(self.expected_bounds_hash)
    }

    pub(crate) fn record_preflight_error(&mut self, error: &AdapterError) {
        if let Some(observed) = crate::ref_action_wait_evidence::observed_bounds_hash(error) {
            self.expected_bounds_hash = Some(observed);
        }
        if let Some(report) = error.details.clone() {
            self.last_report = Some(json!({ "phase": "preflight", "report": report }));
        }
    }

    pub(crate) fn record_resolve_error(&mut self, error: &AdapterError) {
        self.last_report = Some(json!({
            "phase": "resolve",
            "code": error.code.as_str(),
            "message": error.message,
            "details": error.details,
        }));
    }

    pub(crate) fn attach_transient_ambiguity(&self, result: &mut ActionResult) {
        if !self.saw_ambiguity {
            return;
        }
        let mut details = result.details.take().unwrap_or_else(|| json!({}));
        if let Some(object) = details.as_object_mut() {
            object.insert("transient_ambiguity".into(), json!(true));
            result.details = Some(details);
        } else {
            result.details = Some(json!({
                "action_details": details,
                "transient_ambiguity": true,
            }));
        }
    }

    pub(crate) fn attach_wait_metrics(
        &self,
        result: &mut ActionResult,
        lease: &crate::InteractionLease,
        lease_hold_ms: u64,
    ) {
        let mut details = result.details.take().unwrap_or_else(|| json!({}));
        let metrics = json!({
            "read_only_resolve_attempts": self.resolve_attempts,
            "read_only_preflight_attempts": self.preflight_attempts,
            "lease_contention_count": lease.contention_count(),
            "lease_hold_ms": lease_hold_ms,
        });
        if let Some(object) = details.as_object_mut() {
            object.insert("auto_wait".into(), metrics);
            result.details = Some(details);
        } else {
            result.details = Some(json!({
                "action_details": details,
                "auto_wait": metrics,
            }));
        }
    }

    pub(crate) fn attach_error_metrics(&self, mut error: AdapterError) -> AdapterError {
        let mut details = error.details.take().unwrap_or_else(|| json!({}));
        let metrics = json!({
            "read_only_resolve_attempts": self.resolve_attempts,
            "read_only_preflight_attempts": self.preflight_attempts,
        });
        if let Some(object) = details.as_object_mut() {
            object.insert("auto_wait".into(), metrics);
            error.details = Some(details);
        } else {
            error.details = Some(json!({
                "error_details": details,
                "auto_wait": metrics,
            }));
        }
        error
    }
}
