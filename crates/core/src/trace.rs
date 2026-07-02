use crate::error::AppError;
use crate::trace_sanitize::sanitize_trace_value;
use serde_json::{Map, Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const MAX_TRACE_FILE_BYTES: u64 = 64 * 1024 * 1024;

static EVENT_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default)]
enum TracePending {
    #[default]
    None,
    File(PathBuf),
    SegmentDir(PathBuf),
}

#[derive(Debug, Clone, Default)]
enum WriterState {
    #[default]
    Unopened,
    Open(Arc<Mutex<std::fs::File>>),
    Failed,
}

#[derive(Debug, Default)]
struct TraceState {
    pending: TracePending,
    writer: Arc<Mutex<WriterState>>,
    meta_written: AtomicBool,
}

#[derive(Debug, Clone, Default)]
pub struct TraceConfig {
    strict: bool,
    state: Arc<TraceState>,
}

impl TraceConfig {
    pub fn build(
        explicit_path: Option<PathBuf>,
        session_segment_dir: Option<PathBuf>,
        strict: bool,
    ) -> Result<Self, AppError> {
        if strict && explicit_path.is_none() && session_segment_dir.is_none() {
            return Err(AppError::invalid_input_with_suggestion(
                "--trace-strict requires --trace or an active trace-enabled session",
                "Provide --trace <path>, start a session with tracing, or remove --trace-strict.",
            ));
        }
        let (pending, writer) = match explicit_path {
            Some(path) => match open_trace_file(&path) {
                Ok(file) => (
                    TracePending::File(path),
                    WriterState::Open(Arc::new(Mutex::new(file))),
                ),
                Err(err) if strict || err.code() == "INVALID_ARGS" => return Err(err),
                Err(err) => {
                    tracing::warn!("trace open failed: {err}");
                    (TracePending::File(path), WriterState::Failed)
                }
            },
            None => match session_segment_dir {
                Some(dir) => (TracePending::SegmentDir(dir), WriterState::Unopened),
                None => (TracePending::None, WriterState::Unopened),
            },
        };
        Ok(Self {
            strict,
            state: Arc::new(TraceState {
                pending,
                writer: Arc::new(Mutex::new(writer)),
                meta_written: AtomicBool::new(false),
            }),
        })
    }

    pub fn emit(
        &self,
        event: &str,
        session_id: Option<&str>,
        fields: Value,
    ) -> Result<(), AppError> {
        self.emit_lazy(event, session_id, || fields)
    }

    pub fn emit_lazy(
        &self,
        event: &str,
        session_id: Option<&str>,
        fields: impl FnOnce() -> Value,
    ) -> Result<(), AppError> {
        let writer = match self.ensure_writer()? {
            Some(writer) => writer,
            None => return Ok(()),
        };
        self.ensure_meta_if_needed(&writer, session_id)?;
        match writer
            .lock()
            .map_err(|_| AppError::Internal("trace writer lock poisoned".into()))
            .and_then(|mut file| write_event(&mut file, event, session_id, fields()))
        {
            Ok(()) => Ok(()),
            Err(err) if self.strict => Err(err),
            Err(err) => {
                tracing::warn!("trace write failed: {err}");
                Ok(())
            }
        }
    }

    fn ensure_writer(&self) -> Result<Option<Arc<Mutex<std::fs::File>>>, AppError> {
        let mut writer = self
            .state
            .writer
            .lock()
            .map_err(|_| AppError::Internal("trace writer lock poisoned".into()))?;
        match &*writer {
            WriterState::Open(file) => return Ok(Some(file.clone())),
            WriterState::Failed => return Ok(None),
            WriterState::Unopened => {}
        }
        let open_result = match &self.state.pending {
            TracePending::None => {
                *writer = WriterState::Failed;
                return Ok(None);
            }
            TracePending::File(path) => open_trace_file(path),
            TracePending::SegmentDir(dir) => open_segment_trace_file(dir),
        };
        match open_result {
            Ok(file) => {
                let file = Arc::new(Mutex::new(file));
                *writer = WriterState::Open(file.clone());
                Ok(Some(file))
            }
            Err(err) if self.strict => Err(err),
            Err(err) if err.code() == "INVALID_ARGS" => Err(err),
            Err(err) => {
                tracing::warn!("trace open failed: {err}");
                *writer = WriterState::Failed;
                Ok(None)
            }
        }
    }

    pub(crate) fn has_sink(&self) -> bool {
        if matches!(self.state.pending, TracePending::None) {
            return false;
        }
        match self.state.writer.lock() {
            Ok(writer) => !matches!(*writer, WriterState::Failed),
            Err(_) => true,
        }
    }

    pub(crate) fn pending_file_path(&self) -> Option<&Path> {
        match &self.state.pending {
            TracePending::File(path) => Some(path),
            _ => None,
        }
    }

    pub(crate) fn clone_with_session_segment(
        &self,
        session_segment_dir: Option<PathBuf>,
    ) -> Result<Self, AppError> {
        if self.pending_file_path().is_some() {
            return Ok(self.clone());
        }
        match session_segment_dir {
            Some(dir) => Self::build(None, Some(dir), self.strict),
            None => Ok(Self {
                strict: self.strict,
                state: Arc::new(TraceState::default()),
            }),
        }
    }

    fn ensure_meta_if_needed(
        &self,
        writer: &Arc<Mutex<std::fs::File>>,
        session_id: Option<&str>,
    ) -> Result<(), AppError> {
        if self.state.meta_written.load(Ordering::Relaxed) {
            return Ok(());
        }
        let mut file = writer
            .lock()
            .map_err(|_| AppError::Internal("trace writer lock poisoned".into()))?;
        if file.metadata()?.len() > 0 {
            self.state.meta_written.store(true, Ordering::Relaxed);
            return Ok(());
        }
        write_meta_header(&mut file, session_id)?;
        self.state.meta_written.store(true, Ordering::Relaxed);
        Ok(())
    }
}

fn process_segment_suffix() -> &'static str {
    static SUFFIX: OnceLock<String> = OnceLock::new();
    SUFFIX.get_or_init(|| {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("{}-{ts}", std::process::id())
    })
}

pub(crate) fn segment_path_for_dir(dir: &Path) -> PathBuf {
    dir.join(format!("{}.jsonl", process_segment_suffix()))
}

fn open_segment_trace_file(dir: &Path) -> Result<std::fs::File, AppError> {
    ensure_trace_dir(dir)?;
    open_trace_file(&segment_path_for_dir(dir))
}

pub(crate) fn process_start_ms() -> u64 {
    static START_MS: OnceLock<u64> = OnceLock::new();
    *START_MS.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    })
}

fn write_meta_header(file: &mut std::fs::File, session_id: Option<&str>) -> Result<(), AppError> {
    write_event(
        file,
        "trace.meta",
        session_id,
        json!({
            "schema": 1,
            "version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "pid": std::process::id(),
            "proc_start_ms": process_start_ms(),
        }),
    )
}

pub(crate) fn ensure_trace_dir(dir: &Path) -> Result<(), AppError> {
    if let Ok(meta) = std::fs::symlink_metadata(dir) {
        if meta.file_type().is_symlink() {
            return Err(AppError::invalid_input_with_suggestion(
                "Refusing to write trace segments through a symlinked trace directory",
                "Remove the symlink under the session's trace/ directory.",
            ));
        }
    }
    if dir.is_dir() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    Ok(())
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

fn write_event(
    file: &mut std::fs::File,
    event: &str,
    session_id: Option<&str>,
    fields: Value,
) -> Result<(), AppError> {
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
    body.insert(
        "seq".to_string(),
        json!(EVENT_SEQ.fetch_add(1, Ordering::Relaxed)),
    );
    if let Value::Object(fields) = sanitize_trace_value(fields) {
        for (key, value) in fields {
            body.insert(key, value);
        }
    }
    if let Some(sid) = session_id {
        body.insert("session_id".to_string(), json!(sid));
    }
    let mut line = Vec::new();
    serde_json::to_writer(&mut line, &Value::Object(body))?;
    line.push(b'\n');
    file.write_all(&line).map_err(AppError::from)
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

#[cfg(test)]
#[path = "trace_tests.rs"]
mod tests;
