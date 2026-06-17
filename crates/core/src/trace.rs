use crate::error::AppError;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MAX_TRACE_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct TraceConfig {
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
        Ok(Self { strict, writer })
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
}

fn open_trace_file(path: &Path) -> Result<std::fs::File, AppError> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path).map_err(AppError::from)?;
    reject_loose_trace_permissions(&file)?;
    Ok(file)
}

fn write_event(file: &mut std::fs::File, event: &str, fields: Value) -> Result<(), AppError> {
    reject_oversized_trace(file)?;
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

fn reject_oversized_trace(file: &std::fs::File) -> Result<(), AppError> {
    let len = file.metadata()?.len();
    if len < MAX_TRACE_FILE_BYTES {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "Trace file reached the maximum supported size",
        "Start a new --trace file or rotate the existing trace before retrying.",
    ))
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
    [
        "text",
        "value",
        "expected",
        "name",
        "description",
        "label",
        "query",
        "secret",
        "token",
        "password",
        "title",
        "url",
        "help",
        "placeholder",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

fn redacted_value(value: Value) -> Value {
    match value {
        Value::String(text) => json!({
            "redacted": true,
            "chars_bucket": char_count_bucket(text.chars().count())
        }),
        Value::Array(items) => json!({ "redacted": true, "items": items.len() }),
        Value::Object(map) => json!({ "redacted": true, "keys": map.len() }),
        Value::Null => Value::Null,
        _ => json!({ "redacted": true }),
    }
}

fn char_count_bucket(count: usize) -> &'static str {
    match count {
        0 => "0",
        1..=8 => "1-8",
        9..=32 => "9-32",
        33..=128 => "33-128",
        _ => "129+",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn trace_open_rejects_symlink_paths() {
        let base = std::env::temp_dir().join(format!(
            "agent-desktop-trace-symlink-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = base.with_extension("target");
        let link = base.with_extension("link");
        std::fs::write(&target, b"existing").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let result = open_trace_file(&link);

        assert!(result.is_err());
        let _ = std::fs::remove_file(&link);
        let _ = std::fs::remove_file(&target);
    }

    #[test]
    fn trace_redacts_sensitive_fields_but_preserves_messages() {
        let value = sanitize_trace_value(json!({
            "text": "secret",
            "message": "Target is not actionable: supported_action failed",
            "details": { "name": "Private Button" },
            "title": "Window"
        }));

        assert_eq!(value["text"]["redacted"], true);
        assert_eq!(value["details"]["name"]["redacted"], true);
        assert_eq!(value["title"]["redacted"], true);
        assert_eq!(
            value["message"],
            "Target is not actionable: supported_action failed"
        );
    }

    #[test]
    fn trace_redaction_covers_nested_shapes_and_substring_keys() {
        let value = sanitize_trace_value(json!({
            "action": {
                "typed_text": ["secret", "another"],
                "api_token": {"kind": "bearer"},
                "password": null,
                "counter": 3
            }
        }));

        assert_eq!(value["action"]["typed_text"]["redacted"], true);
        assert_eq!(value["action"]["typed_text"]["items"], 2);
        assert_eq!(value["action"]["api_token"]["redacted"], true);
        assert_eq!(value["action"]["api_token"]["keys"], 1);
        assert!(value["action"]["password"].is_null());
        assert_eq!(value["action"]["counter"], 3);
        assert_eq!(char_count_bucket(0), "0");
        assert_eq!(char_count_bucket(8), "1-8");
        assert_eq!(char_count_bucket(65), "33-128");
    }

    #[test]
    fn trace_write_rejects_files_at_size_cap() {
        let path = std::env::temp_dir().join(format!(
            "agent-desktop-trace-cap-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_TRACE_FILE_BYTES).unwrap();
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let mut file = open_trace_file(&path).unwrap();

        let err = write_event(&mut file, "event", json!({})).unwrap_err();

        assert_eq!(err.code(), "INVALID_ARGS");
        let _ = std::fs::remove_file(path);
    }
}
