use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct Response {
    pub version: &'static str,
    pub ok: bool,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<AppContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
}

#[derive(Debug, Serialize)]
pub struct AppContext {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowContext>,
}

#[derive(Debug, Serialize)]
pub struct WindowContext {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_detail: Option<String>,
}

impl Response {
    pub fn ok(command: impl Into<String>, data: Value) -> Self {
        Self {
            version: "1.0",
            ok: true,
            command: command.into(),
            app: None,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(command: impl Into<String>, payload: ErrorPayload) -> Self {
        Self {
            version: "1.0",
            ok: false,
            command: command.into(),
            app: None,
            data: None,
            error: Some(payload),
        }
    }

    pub fn with_app(mut self, ctx: AppContext) -> Self {
        self.app = Some(ctx);
        self
    }
}

impl ErrorPayload {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            suggestion: None,
            retry_command: None,
            platform_detail: None,
        }
    }

    pub fn with_suggestion(mut self, s: impl Into<String>) -> Self {
        self.suggestion = Some(s.into());
        self
    }

    pub fn with_retry(mut self, cmd: impl Into<String>) -> Self {
        self.retry_command = Some(cmd.into());
        self
    }
}
