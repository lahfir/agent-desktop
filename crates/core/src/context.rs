use crate::{error::AppError, trace::TraceConfig};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct CommandContext {
    pub session_id: Option<String>,
    pub trace: TraceConfig,
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
        Ok(Self {
            session_id,
            trace: TraceConfig::new(trace_path, trace_strict)?,
        })
    }

    pub fn for_batch_item(&self, session_id: Option<String>) -> Result<Self, AppError> {
        let session_id = session_id.or_else(|| self.session_id.clone());
        if let Some(id) = session_id.as_deref() {
            validate_session_id(id)?;
        }
        Ok(Self {
            session_id,
            trace: self.trace.clone(),
        })
    }

    pub fn trace(&self, event: &str, fields: Value) -> Result<(), AppError> {
        self.trace.emit(event, fields)
    }
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
mod tests {
    use super::*;

    #[test]
    fn accepts_filesystem_safe_session_ids() {
        assert!(validate_session_id("agent-1_A").is_ok());
    }

    #[test]
    fn rejects_path_like_session_ids() {
        assert!(validate_session_id("../agent").is_err());
        assert!(validate_session_id("agent/a").is_err());
    }

    #[test]
    fn trace_writes_jsonl_without_stdout_dependency() {
        let path = std::env::temp_dir().join(format!(
            "agent-desktop-trace-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let context = CommandContext::new(None, Some(path.clone()), false).unwrap();

        context
            .trace("ref.resolve.ok", serde_json::json!({ "ref": "@e1" }))
            .unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        let event: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
        assert_eq!(event["event"], "ref.resolve.ok");
        assert_eq!(event["ref"], "@e1");
        assert!(event["ts_ms"].as_u64().is_some());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn trace_write_failure_is_best_effort_unless_strict() {
        let missing = std::env::temp_dir()
            .join("agent-desktop-missing-dir")
            .join("trace.jsonl");

        let best_effort = CommandContext::new(None, Some(missing.clone()), false).unwrap();
        assert!(best_effort.trace("event", serde_json::json!({})).is_ok());

        assert!(CommandContext::new(None, Some(missing), true).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn trace_file_is_private_on_create() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "agent-desktop-private-trace-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let context = CommandContext::new(None, Some(path.clone()), true).unwrap();
        context.trace("event", serde_json::json!({})).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn trace_strict_requires_trace_path() {
        let err = CommandContext::new(None, None, true).unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGS");
    }

    #[test]
    fn batch_item_inherits_or_overrides_session_without_trace_loss() {
        let parent = CommandContext::new(
            Some("parent".into()),
            Some(std::env::temp_dir().join("agent-desktop-context-test.jsonl")),
            false,
        )
        .unwrap();

        let inherited = parent.for_batch_item(None).unwrap();
        let overridden = parent.for_batch_item(Some("child".into())).unwrap();

        assert_eq!(inherited.session_id.as_deref(), Some("parent"));
        assert_eq!(overridden.session_id.as_deref(), Some("child"));
        assert!(overridden.trace.path.is_some());
    }
}
