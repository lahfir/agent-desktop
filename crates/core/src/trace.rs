use crate::error::AppError;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct TraceConfig {
    pub path: Option<PathBuf>,
    pub strict: bool,
}

impl TraceConfig {
    pub fn new(path: Option<PathBuf>, strict: bool) -> Result<Self, AppError> {
        if strict && path.is_none() {
            return Err(AppError::invalid_input_with_suggestion(
                "--trace-strict requires --trace",
                "Provide --trace <path> or remove --trace-strict.",
            ));
        }
        Ok(Self { path, strict })
    }

    pub fn emit(&self, event: &str, fields: Value) -> Result<(), AppError> {
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        match write_event(path, event, fields) {
            Ok(()) => Ok(()),
            Err(err) if self.strict => Err(err),
            Err(err) => {
                tracing::warn!("trace write failed: {err}");
                Ok(())
            }
        }
    }
}

fn write_event(path: &Path, event: &str, fields: Value) -> Result<(), AppError> {
    let mut body = Map::new();
    body.insert("event".to_string(), json!(event));
    body.insert(
        "ts_ms".to_string(),
        json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| AppError::Internal(err.to_string()))?
                .as_millis()
        ),
    );
    if let Value::Object(fields) = fields {
        for (key, value) in fields {
            body.insert(key, value);
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(AppError::from)?;
    serde_json::to_writer(&mut file, &Value::Object(body))?;
    use std::io::Write;
    file.write_all(b"\n").map_err(AppError::from)
}
