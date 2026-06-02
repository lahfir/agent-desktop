use crate::error::AppError;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct TraceConfig {
    pub path: Option<PathBuf>,
    pub strict: bool,
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
        let Some(writer) = self.writer.as_ref() else {
            return Ok(());
        };
        match writer
            .lock()
            .map_err(|_| AppError::Internal("trace writer lock poisoned".into()))
            .and_then(|mut file| write_event(&mut file, event, fields))
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
    }
    options.open(path).map_err(AppError::from)
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
    if let Value::Object(fields) = fields {
        for (key, value) in fields {
            body.insert(key, value);
        }
    }
    serde_json::to_writer(&mut *file, &Value::Object(body))?;
    use std::io::Write;
    file.write_all(b"\n").map_err(AppError::from)
}
