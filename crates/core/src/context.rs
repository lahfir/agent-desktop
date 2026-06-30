use crate::{
    action::Action, action_request::ActionRequest, error::AppError,
    interaction_policy::InteractionPolicy, trace::TraceConfig,
};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct WaitSelector {
    pub query_raw: String,
    pub gone: bool,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct CommandContext {
    session_id: Option<String>,
    trace: TraceConfig,
    headed: bool,
    wait_selector: Option<WaitSelector>,
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
            headed: false,
            wait_selector: None,
        })
    }

    /// Selects headed interaction: ref actions may move the cursor and steal
    /// focus, unlocking the physical click/scroll/keypress fallbacks in the
    /// action chain. Off by default — the tool is headless-first (Playwright
    /// style: headless is the default, headed is opt-in via `--headed`).
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

    /// Builds the action request for a ref command. Headless (default) uses the
    /// command's own `base` policy — its minimum viable policy with no cursor
    /// movement (most commands are pure-AX `headless`; `type` is `focus_fallback`
    /// because typing requires focus). `--headed` upgrades any base to `headed`,
    /// unlocking the cursor/OS-input fallbacks instead of failing closed.
    pub fn request(&self, action: Action, base: InteractionPolicy) -> ActionRequest {
        ActionRequest {
            action,
            policy: self.policy_with_base(base),
        }
    }

    /// Builds the action request using the action's canonical CLI base policy
    /// from `Action::base_interaction_policy()`, ensuring a single source of
    /// truth for the per-action minimum. `--headed` upgrades the base to
    /// `headed` exactly as `request()` does.
    pub fn request_base(&self, action: Action) -> ActionRequest {
        let base = action.base_interaction_policy();
        self.request(action, base)
    }

    /// Policy for raw physical-input commands (hover, drag, mouse-*). They
    /// have no semantic action chain, so headless denies both focus stealing
    /// and cursor movement unless `--headed` opts in.
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
        Ok(Self {
            session_id,
            trace: self.trace.clone(),
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
        assert!(
            event.get("session_id").is_none(),
            "session_id must be absent when not set"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn trace_injects_session_id_as_top_level_unredacted_field() {
        let path = std::env::temp_dir().join(format!(
            "agent-desktop-session-trace-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let context =
            CommandContext::new(Some("my-session".into()), Some(path.clone()), false).unwrap();

        context
            .trace("ref.resolve.ok", serde_json::json!({ "ref": "@e1" }))
            .unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        let event: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
        assert_eq!(
            event["session_id"], "my-session",
            "session_id must be a top-level string"
        );
        assert_eq!(event["event"], "ref.resolve.ok");
        assert!(event["ts_ms"].as_u64().is_some());
        assert!(
            !body.contains("redacted"),
            "session_id must not be redacted"
        );
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

    #[test]
    fn trace_lazy_does_not_build_fields_when_trace_is_disabled() {
        let context = CommandContext::default();
        let built = std::cell::Cell::new(false);

        context
            .trace_lazy("event", || {
                built.set(true);
                serde_json::json!({})
            })
            .unwrap();

        assert!(!built.get());
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

    #[cfg(unix)]
    #[test]
    fn trace_rejects_loose_existing_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "agent-desktop-loose-trace-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, "").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let err = CommandContext::new(None, Some(path.clone()), false).unwrap_err();

        assert_eq!(err.code(), "INVALID_ARGS");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn trace_redacts_sensitive_text_and_value_fields() {
        let path = std::env::temp_dir().join(format!(
            "agent-desktop-redacted-trace-{}.jsonl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let context = CommandContext::new(None, Some(path.clone()), true).unwrap();
        context
            .trace(
                "event",
                serde_json::json!({
                    "text": "secret",
                    "value": "hidden",
                    "name": "private label",
                    "description": "private desc",
                    "message": "diagnostic error",
                    "post_state": { "value": "deep secret" },
                    "target_label": "button secret",
                    "nested": { "expected": "token" },
                    "title": "private window title",
                    "url": "https://internal.example/doc",
                    "help": "private tooltip",
                    "placeholder": "private placeholder"
                }),
            )
            .unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        let event: serde_json::Value = serde_json::from_str(body.trim()).unwrap();
        assert_eq!(event["text"]["redacted"], true);
        assert_eq!(event["value"]["redacted"], true);
        assert_eq!(event["name"]["redacted"], true);
        assert_eq!(event["description"]["redacted"], true);
        assert_eq!(event["message"], "diagnostic error");
        assert_eq!(event["post_state"]["value"]["redacted"], true);
        assert_eq!(event["target_label"]["redacted"], true);
        assert_eq!(event["nested"]["expected"]["redacted"], true);
        assert_eq!(event["title"]["redacted"], true);
        assert_eq!(event["url"]["redacted"], true);
        assert_eq!(event["help"]["redacted"], true);
        assert_eq!(event["placeholder"]["redacted"], true);
        assert!(!body.contains("secret"));
        assert!(!body.contains("hidden"));
        assert!(!body.contains("private label"));
        assert!(!body.contains("private desc"));
        assert!(!body.contains("token"));
        assert!(!body.contains("private window title"));
        assert!(!body.contains("internal.example"));
        assert!(!body.contains("private tooltip"));
        assert!(!body.contains("private placeholder"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn trace_strict_requires_trace_path() {
        let err = CommandContext::new(None, None, true).unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGS");
    }

    #[test]
    fn batch_item_clears_wait_selector() {
        let parent = CommandContext::default().with_wait_selector(Some(WaitSelector {
            query_raw: "button:OK".into(),
            gone: false,
            timeout_ms: 5_000,
        }));
        let child = parent.for_batch_item(None).unwrap();
        assert!(child.wait_selector().is_none());
    }

    #[test]
    fn batch_item_inherits_or_overrides_session_without_trace_loss() {
        let path = std::env::temp_dir().join("agent-desktop-context-test.jsonl");
        let _ = std::fs::remove_file(&path);
        let parent = CommandContext::new(Some("parent".into()), Some(path.clone()), false).unwrap();

        let inherited = parent.for_batch_item(None).unwrap();
        let overridden = parent.for_batch_item(Some("child".into())).unwrap();

        assert_eq!(inherited.session_id(), Some("parent"));
        assert_eq!(overridden.session_id(), Some("child"));
        overridden
            .trace("batch.child", serde_json::json!({ "ok": true }))
            .unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("batch.child"));
        let _ = std::fs::remove_file(path);
    }
}
