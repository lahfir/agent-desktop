use crate::error::AppError;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct TraceConfig {
    path: Option<PathBuf>,
    strict: bool,
    writer: Option<Arc<Mutex<std::fs::File>>>,
}

impl TraceConfig {
    pub fn new(path: Option<PathBuf>, strict: bool) -> Result<Self, AppError> {
        if strict && path.is_none() {
            return Err(AppError::invalid_input_with_suggestion(
                "--trace-strict requires --trace",
                "Provide --trace <path> or remove --trace-strict.",
            ));
        }
        let writer = match path.as_deref() {
            Some(path) => match open_trace_file(path) {
                Ok(file) => Some(Arc::new(Mutex::new(file))),
                Err(err) if err.code() == "INVALID_ARGS" => return Err(err),
                Err(err) if strict => return Err(err),
                Err(err) => {
                    tracing::warn!("trace open failed: {err}");
                    None
                }
            },
            None => None,
        };
        Ok(Self {
            path,
            strict,
            writer,
        })
    }

    pub fn emit(&self, event: &str, fields: Value) -> Result<(), AppError> {
        self.emit_lazy(event, || fields)
    }

    pub fn emit_lazy(&self, event: &str, fields: impl FnOnce() -> Value) -> Result<(), AppError> {
        let Some(writer) = self.writer.as_ref() else {
            return Ok(());
        };
        match writer
            .lock()
            .map_err(|_| AppError::Internal("trace writer lock poisoned".into()))
            .and_then(|mut file| write_event(&mut file, event, fields()))
        {
            Ok(()) => Ok(()),
            Err(err) if self.strict => Err(err),
            Err(err) => {
                tracing::warn!("trace write failed: {err}");
                Ok(())
            }
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

fn open_trace_file(path: &Path) -> Result<std::fs::File, AppError> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(AppError::from)?;
    reject_loose_trace_permissions(&file)?;
    Ok(file)
}

fn write_event(file: &mut std::fs::File, event: &str, fields: Value) -> Result<(), AppError> {
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
    if let Value::Object(fields) = sanitize_trace_value(fields) {
        for (key, value) in fields {
            body.insert(key, value);
        }
    }
    serde_json::to_writer(&mut *file, &Value::Object(body))?;
    use std::io::Write;
    file.write_all(b"\n").map_err(AppError::from)
}

#[cfg(unix)]
fn reject_loose_trace_permissions(file: &std::fs::File) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = file.metadata()?.permissions().mode() & 0o777;
    if mode & 0o077 == 0 {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "Trace file must not be readable or writable by group/other",
        "Use a new --trace path or run chmod 600 on the existing trace file.",
    ))
}

#[cfg(not(unix))]
fn reject_loose_trace_permissions(_file: &std::fs::File) -> Result<(), AppError> {
    Ok(())
}

fn sanitize_trace_value(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    if is_sensitive_trace_key(&key) {
                        (key, redacted_value(value))
                    } else {
                        (key, sanitize_trace_value(value))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(sanitize_trace_value).collect()),
        other => other,
    }
}

fn is_sensitive_trace_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "text" | "value" | "expected" | "name" | "description" | "message"
    )
}

fn redacted_value(value: Value) -> Value {
    match value {
        Value::String(text) => json!({ "redacted": true, "chars": text.chars().count() }),
        Value::Array(items) => json!({ "redacted": true, "items": items.len() }),
        Value::Object(map) => json!({ "redacted": true, "keys": map.len() }),
        Value::Null => Value::Null,
        _ => json!({ "redacted": true }),
    }
}
