use crate::{
    action::Action, action_request::ActionRequest, error::AppError,
    interaction_policy::InteractionPolicy, session, trace::TraceConfig,
};
use serde_json::{Value, json};
use std::cell::Cell;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct CommandContext {
    session_id: Option<String>,
    trace: TraceConfig,
    artifacts_full: bool,
    headed: bool,
    wait_selector: Option<WaitSelector>,
}

#[derive(Debug, Clone)]
pub struct WaitSelector {
    pub query_raw: String,
    pub gone: bool,
    pub timeout_ms: u64,
}

pub struct CommandScope<'a> {
    context: &'a CommandContext,
    command: &'static str,
    started: Instant,
    finished: Cell<bool>,
}

impl CommandScope<'_> {
    pub fn complete(self, result: &Result<Value, AppError>) {
        self.finished.set(true);
        let duration_ms = self.started.elapsed().as_millis();
        match result {
            Ok(_) => {
                let _ = self.context.trace(
                    "command.end",
                    json!({
                        "command": self.command,
                        "ok": true,
                        "duration_ms": duration_ms,
                    }),
                );
            }
            Err(err) => {
                let _ = self.context.trace(
                    "command.end",
                    json!({
                        "command": self.command,
                        "ok": false,
                        "duration_ms": duration_ms,
                        "code": err.code(),
                        "message": err.to_string(),
                    }),
                );
            }
        }
    }
}

impl Drop for CommandScope<'_> {
    fn drop(&mut self) {
        if self.finished.get() {
            return;
        }
        let _ = self.context.trace(
            "command.end",
            json!({
                "command": self.command,
                "ok": false,
                "duration_ms": self.started.elapsed().as_millis(),
                "code": "INTERNAL",
                "message": "command scope dropped without completion",
            }),
        );
    }
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
        let (segment_dir, artifacts_full) =
            session_trace_state(session_id.as_deref(), trace_path.is_some())?;
        Ok(Self {
            session_id,
            trace: TraceConfig::build(trace_path, segment_dir, trace_strict)?,
            artifacts_full,
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

    pub fn command_scope(&self, command: &'static str) -> CommandScope<'_> {
        let _ = self.trace("command.start", json!({ "command": command }));
        CommandScope {
            context: self,
            command,
            started: Instant::now(),
            finished: Cell::new(false),
        }
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
        let (segment_dir, artifacts_full) = session_trace_state(
            session_id.as_deref(),
            self.trace.pending_file_path().is_some(),
        )?;
        let trace = if self.trace.pending_file_path().is_some() || session_id == self.session_id {
            self.trace.clone()
        } else {
            self.trace.clone_with_session_segment(segment_dir)?
        };
        Ok(Self {
            session_id,
            trace,
            artifacts_full,
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

    pub fn artifacts_full(&self) -> bool {
        self.artifacts_full
    }
}

fn session_trace_state(
    session_id: Option<&str>,
    explicit_trace: bool,
) -> Result<(Option<PathBuf>, bool), AppError> {
    if explicit_trace {
        return Ok((None, false));
    }
    let Some(session_id) = session_id else {
        return Ok((None, false));
    };
    let manifest = session::read_manifest(session_id)?;
    let trace_dir = if manifest.as_ref().is_some_and(|m| m.trace_enabled()) {
        Some(session::trace_dir(session_id)?)
    } else {
        None
    };
    let artifacts_full = manifest.as_ref().is_some_and(|m| m.artifacts_full());
    Ok((trace_dir, artifacts_full))
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
