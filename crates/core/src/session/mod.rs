mod gc;
mod liveness;
mod manifest;

pub use gc::{GcOptions, GcReport, gc, is_live};
pub use liveness::SessionLivenessLease;
pub use manifest::{ArtifactsMode, SessionManifest, SessionTraceMode};

use crate::{
    AppError, context::validate_session_id, refs::write_private_file, refs_store::RefStore,
};
use serde_json;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SESSION_MANIFEST_FILE: &str = "session.json";
const MAX_SESSION_MANIFEST_BYTES: u64 = 64 * 1024;
pub(super) const TRACE_LIVENESS_WINDOW: Duration = Duration::from_secs(300);
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct StartSessionOptions {
    pub name: Option<String>,
    pub trace: SessionTraceMode,
    pub artifacts: ArtifactsMode,
}

impl Default for StartSessionOptions {
    fn default() -> Self {
        Self {
            name: None,
            trace: SessionTraceMode::On,
            artifacts: ArtifactsMode::Events,
        }
    }
}

pub fn agent_desktop_dir() -> Result<PathBuf, AppError> {
    crate::state_root::resolve_configured_state_root()
}

pub fn session_dir(session_id: &str) -> Result<PathBuf, AppError> {
    validate_session_id(session_id)?;
    Ok(agent_desktop_dir()?.join("sessions").join(session_id))
}

pub fn trace_dir(session_id: &str) -> Result<PathBuf, AppError> {
    Ok(RefStore::for_session(Some(session_id))?.trace_dir())
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
    Ok(None)
}

pub fn read_manifest(session_id: &str) -> Result<Option<SessionManifest>, AppError> {
    let path = manifest_path(session_id)?;
    let json = match crate::private_file::read_private_bounded(&path, MAX_SESSION_MANIFEST_BYTES) {
        Ok(json) => json,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Ok(ignore_unreadable_manifest(&path, &err)),
    };
    match serde_json::from_slice(&json) {
        Ok(manifest) => Ok(Some(manifest)),
        Err(err) => Ok(ignore_unreadable_manifest(&path, &err)),
    }
}

fn ignore_unreadable_manifest<E: std::fmt::Display>(
    path: &Path,
    err: &E,
) -> Option<SessionManifest> {
    tracing::warn!(
        "ignoring unreadable session manifest {}: {err}",
        path.display()
    );
    None
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
    if matches!(options.trace, SessionTraceMode::Off)
        && matches!(options.artifacts, ArtifactsMode::Full)
    {
        return Err(AppError::invalid_input_with_suggestion(
            "Artifacts mode full requires tracing",
            "Remove --no-trace or omit --screenshots.",
        ));
    }
    let id = new_session_id();
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
        artifacts: options.artifacts,
    };
    write_manifest(&manifest)?;
    Ok(manifest)
}

pub fn end_session(session_id: &str) -> Result<SessionManifest, AppError> {
    validate_session_id(session_id)?;
    let _lease = acquire_liveness_lease(session_id)?;
    let id = session_id.to_string();
    let mut manifest = read_manifest(&id)?.ok_or_else(|| {
        AppError::invalid_input_with_suggestion(
            format!("Session '{id}' has no manifest"),
            "Use `session list` to see known sessions.",
        )
    })?;
    if manifest.ended_at.is_none() {
        manifest.ended_at = Some(now_millis());
        write_manifest(&manifest)?;
        if manifest.artifacts == ArtifactsMode::Full
            && let Ok(store) = crate::refs_store::RefStore::for_session(Some(&id))
        {
            store.discard_duplicated_ref_scaffolding();
        }
    }
    Ok(manifest)
}

pub fn acquire_liveness_lease(session_id: &str) -> Result<Option<SessionLivenessLease>, AppError> {
    acquire_liveness_lease_with_deadline(session_id, crate::Deadline::standard()?)
}

pub(crate) fn acquire_liveness_lease_with_deadline(
    session_id: &str,
    deadline: crate::Deadline,
) -> Result<Option<SessionLivenessLease>, AppError> {
    validate_session_id(session_id)?;
    liveness::acquire(session_id, deadline)
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

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "session_gc_tests.rs"]
mod gc_tests;
