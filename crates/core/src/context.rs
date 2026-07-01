use crate::{
    action::Action, action_request::ActionRequest, error::AppError,
    interaction_policy::InteractionPolicy, session, trace::TraceConfig,
};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct CommandContext {
    session_id: Option<String>,
    trace: TraceConfig,
    headed: bool,
    wait_selector: Option<WaitSelector>,
}

#[derive(Debug, Clone)]
pub struct WaitSelector {
    pub query_raw: String,
    pub gone: bool,
    pub timeout_ms: u64,
}

impl CommandContext {
    pub fn new(
        session_id: Option<String>,
        trace_path: Option<PathBuf>,
        trace_strict: bool,
    ) -> Result<Self, AppError> {
        if let Some(id) = session_id.as_deref() {
            validate_session_id(id)?;
        }
        let segment_dir = session_segment_dir(session_id.as_deref(), trace_path.is_some())?;
        Ok(Self {
            session_id,
            trace: TraceConfig::build(trace_path, segment_dir, trace_strict)?,
            headed: false,
            wait_selector: None,
        })
    }

    pub fn with_headed(mut self, headed: bool) -> Self {
        self.headed = headed;
        self
    }

    pub fn with_wait_selector(mut self, wait_selector: Option<WaitSelector>) -> Self {
        self.wait_selector = wait_selector;
        self
    }

    pub fn wait_selector(&self) -> Option<&WaitSelector> {
        self.wait_selector.as_ref()
    }

    pub fn request(&self, action: Action, base: InteractionPolicy) -> ActionRequest {
        ActionRequest {
            action,
            policy: self.policy_with_base(base),
        }
    }

    pub fn request_base(&self, action: Action) -> ActionRequest {
        let base = action.base_interaction_policy();
        self.request(action, base)
    }

    pub fn physical_input_policy(&self) -> InteractionPolicy {
        self.policy_with_base(InteractionPolicy::headless())
    }

    fn policy_with_base(&self, base: InteractionPolicy) -> InteractionPolicy {
        if self.headed {
            InteractionPolicy::headed()
        } else {
            base
        }
    }

    pub fn for_batch_item(&self, session_id: Option<String>) -> Result<Self, AppError> {
        let session_id = session_id.or_else(|| self.session_id.clone());
        if let Some(id) = session_id.as_deref() {
            validate_session_id(id)?;
        }
        let trace = if self.trace.pending_file_path().is_some() {
            self.trace.clone()
        } else {
            let segment_dir = session_segment_dir(session_id.as_deref(), false)?;
            self.trace.clone_with_session_segment(segment_dir)?
        };
        Ok(Self {
            session_id,
            trace,
            headed: self.headed,
            wait_selector: None,
        })
    }

    pub fn trace(&self, event: &str, fields: Value) -> Result<(), AppError> {
        self.trace.emit(event, self.session_id.as_deref(), fields)
    }

    pub fn trace_lazy(&self, event: &str, fields: impl FnOnce() -> Value) -> Result<(), AppError> {
        self.trace
            .emit_lazy(event, self.session_id.as_deref(), fields)
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn trace_enabled(&self) -> bool {
        self.trace.has_sink()
    }
}

fn session_segment_dir(
    session_id: Option<&str>,
    explicit_trace: bool,
) -> Result<Option<PathBuf>, AppError> {
    if explicit_trace {
        return Ok(None);
    }
    let Some(session_id) = session_id else {
        return Ok(None);
    };
    if !session::trace_enabled_for_session(session_id)? {
        return Ok(None);
    }
    Ok(Some(session::trace_dir(session_id)?))
}

pub fn validate_session_id(id: &str) -> Result<(), AppError> {
    let valid = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'));
    if valid {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "Session id must be 1-64 chars using letters, numbers, '-' or '_'",
        "Use a short filesystem-safe session id such as run_1 or agent-a.",
    ))
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
