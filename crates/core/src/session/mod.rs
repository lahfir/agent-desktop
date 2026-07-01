mod gc;
mod manifest;

pub use gc::{GcOptions, GcReport, gc, is_live, pointer_references_live_session};
pub use manifest::{SessionManifest, SessionTraceMode};

use crate::{
    context::validate_session_id,
    error::AppError,
    refs::{home_dir, write_private_file},
    refs_store::RefStore,
};
use serde_json;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CURRENT_SESSION_FILE: &str = "current_session";
const SESSION_MANIFEST_FILE: &str = "session.json";
pub(super) const TRACE_LIVENESS_WINDOW: Duration = Duration::from_secs(300);
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct StartSessionOptions {
    pub name: Option<String>,
    pub trace: SessionTraceMode,
    pub force: bool,
}

pub fn agent_desktop_dir() -> Result<PathBuf, AppError> {
    let home = home_dir().ok_or_else(|| AppError::Internal("HOME directory not found".into()))?;
    Ok(home.join(".agent-desktop"))
}

pub fn session_dir(session_id: &str) -> Result<PathBuf, AppError> {
    validate_session_id(session_id)?;
    Ok(agent_desktop_dir()?.join("sessions").join(session_id))
}

pub fn trace_dir(session_id: &str) -> Result<PathBuf, AppError> {
    Ok(RefStore::for_session(Some(session_id))?.trace_dir())
}

pub fn current_session_path() -> Result<PathBuf, AppError> {
    Ok(agent_desktop_dir()?.join(CURRENT_SESSION_FILE))
}

pub fn resolve_active_session(
    explicit: Option<&str>,
    env: Option<&str>,
) -> Result<Option<String>, AppError> {
    if let Some(id) = explicit {
        validate_session_id(id)?;
        return Ok(Some(id.to_string()));
    }
    if let Some(id) = env {
        if id.is_empty() {
            return Err(AppError::invalid_input_with_suggestion(
                "AGENT_DESKTOP_SESSION must not be empty",
                "Unset the variable or set it to a valid session id.",
            ));
        }
        validate_session_id(id)?;
        return Ok(Some(id.to_string()));
    }
    read_current_session_pointer()
}

pub fn read_current_session_pointer() -> Result<Option<String>, AppError> {
    let path = current_session_path()?;
    let mut file = match open_session_file(&path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut id = String::new();
    file.read_to_string(&mut id)?;
    let id = id.trim().to_string();
    if id.is_empty() {
        return Ok(None);
    }
    validate_session_id(&id)?;
    Ok(Some(id))
}

pub fn write_current_session_pointer(session_id: &str) -> Result<(), AppError> {
    validate_session_id(session_id)?;
    write_private_file(&current_session_path()?, session_id.as_bytes())
}

pub fn clear_current_session_pointer() -> Result<(), AppError> {
    match std::fs::remove_file(current_session_path()?) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

pub fn read_manifest(session_id: &str) -> Result<Option<SessionManifest>, AppError> {
    let path = manifest_path(session_id)?;
    let mut file = match open_session_file(&path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let mut json = String::new();
    file.read_to_string(&mut json)?;
    match serde_json::from_str(&json) {
        Ok(manifest) => Ok(Some(manifest)),
        Err(err) => {
            tracing::warn!(
                "ignoring unreadable session manifest {}: {err}",
                path.display()
            );
            Ok(None)
        }
    }
}

pub fn write_manifest(manifest: &SessionManifest) -> Result<(), AppError> {
    validate_session_id(&manifest.id)?;
    let json = serde_json::to_string_pretty(manifest)?;
    write_private_file(&manifest_path(&manifest.id)?, json.as_bytes())
}

pub fn trace_enabled_for_session(session_id: &str) -> Result<bool, AppError> {
    Ok(read_manifest(session_id)?.is_some_and(|manifest| manifest.trace_enabled()))
}

pub fn new_session_id() -> String {
    let n = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    format!("run-{}-{pid}-{n}", now_millis())
}

pub fn validate_session_name(name: &str) -> Result<String, AppError> {
    if name.is_empty() {
        return Err(AppError::invalid_input_with_suggestion(
            "Session name must not be empty",
            "Omit --name or provide a short descriptive label.",
        ));
    }
    if name.len() > 128 {
        return Err(AppError::invalid_input_with_suggestion(
            "Session name must be at most 128 characters",
            "Use a shorter session name.",
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(AppError::invalid_input_with_suggestion(
            "Session name must not contain control characters",
            "Use printable ASCII or Unicode text for --name.",
        ));
    }
    Ok(name.to_string())
}

pub fn list_sessions() -> Result<Vec<SessionManifest>, AppError> {
    let sessions_root = agent_desktop_dir()?.join("sessions");
    let Ok(entries) = std::fs::read_dir(sessions_root) else {
        return Ok(Vec::new());
    };
    let mut manifests = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if validate_session_id(name).is_err() {
            continue;
        }
        if let Some(manifest) = read_manifest(name)? {
            manifests.push(manifest);
        }
    }
    manifests.sort_by_key(|manifest| manifest.created_at);
    Ok(manifests)
}

pub fn start_session(options: StartSessionOptions) -> Result<SessionManifest, AppError> {
    if !options.force && pointer_references_live_session()? {
        return Err(AppError::invalid_input_with_suggestion(
            "Refusing to clobber the current session pointer while it references a live session",
            "Run `session end` first, set AGENT_DESKTOP_SESSION for concurrent work, or pass --force.",
        ));
    }
    let id = new_session_id();
    validate_session_id(&id)?;
    let name = options
        .name
        .map(|name| validate_session_name(&name))
        .transpose()?;
    let dir = session_dir(&id)?;
    create_session_tree(&dir)?;
    let manifest = SessionManifest {
        id: id.clone(),
        name,
        created_at: now_millis(),
        ended_at: None,
        trace: options.trace,
    };
    write_manifest(&manifest)?;
    write_current_session_pointer(&id)?;
    Ok(manifest)
}

pub fn end_session(session_id: Option<&str>) -> Result<SessionManifest, AppError> {
    let id = match session_id {
        Some(id) => {
            validate_session_id(id)?;
            id.to_string()
        }
        None => read_current_session_pointer()?.ok_or_else(|| {
            AppError::invalid_input_with_suggestion(
                "No active session to end",
                "Pass a session id or run `session start` first.",
            )
        })?,
    };
    let mut manifest = read_manifest(&id)?.ok_or_else(|| {
        AppError::invalid_input_with_suggestion(
            format!("Session '{id}' has no manifest"),
            "Use `session list` to see known sessions.",
        )
    })?;
    if manifest.ended_at.is_none() {
        manifest.ended_at = Some(now_millis());
        write_manifest(&manifest)?;
    }
    if read_current_session_pointer()?.as_deref() == Some(id.as_str()) {
        clear_current_session_pointer()?;
    }
    Ok(manifest)
}

fn create_session_tree(dir: &Path) -> Result<(), AppError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir.join("trace"))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir.join("trace"))?;
    }
    Ok(())
}

fn manifest_path(session_id: &str) -> Result<PathBuf, AppError> {
    Ok(session_dir(session_id)?.join(SESSION_MANIFEST_FILE))
}

pub(super) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn open_session_file(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
    }
    #[cfg(not(unix))]
    {
        if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "session path must not be a symlink",
            ));
        }
        std::fs::File::open(path)
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
