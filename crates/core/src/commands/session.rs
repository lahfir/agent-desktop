use crate::error::AppError;
use crate::session::{
    ArtifactsMode, GcOptions, SessionTraceMode, StartSessionOptions, end_session, gc,
    list_sessions, start_session,
};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum SessionAction {
    Start {
        name: Option<String>,
        no_trace: bool,
        screenshots: bool,
        force: bool,
    },
    End {
        id: Option<String>,
    },
    List,
    Gc {
        older_than_secs: Option<u64>,
        ended_only: bool,
    },
}

pub fn execute(action: SessionAction) -> Result<Value, AppError> {
    match action {
        SessionAction::Start {
            name,
            no_trace,
            screenshots,
            force,
        } => {
            let manifest = start_session(StartSessionOptions {
                name,
                trace: if no_trace {
                    SessionTraceMode::Off
                } else {
                    SessionTraceMode::On
                },
                artifacts: if screenshots {
                    ArtifactsMode::Full
                } else {
                    ArtifactsMode::Events
                },
                force,
            })?;
            Ok(json!({
                "session_id": manifest.id,
                "name": manifest.name,
                "trace": manifest.trace,
                "artifacts": manifest.artifacts,
                "created_at": manifest.created_at,
            }))
        }
        SessionAction::End { id } => {
            let manifest = end_session(id.as_deref())?;
            Ok(json!({
                "session_id": manifest.id,
                "ended_at": manifest.ended_at,
            }))
        }
        SessionAction::List => {
            let sessions: Vec<Value> = list_sessions()?
                .into_iter()
                .map(|manifest| {
                    json!({
                        "session_id": manifest.id,
                        "name": manifest.name,
                        "created_at": manifest.created_at,
                        "ended_at": manifest.ended_at,
                        "trace": manifest.trace,
                        "artifacts": manifest.artifacts,
                    })
                })
                .collect();
            Ok(json!({ "sessions": sessions }))
        }
        SessionAction::Gc {
            older_than_secs,
            ended_only,
        } => {
            let report = gc(GcOptions {
                ended_only,
                older_than: older_than_secs.map(Duration::from_secs),
            })?;
            Ok(json!({ "removed": report.removed }))
        }
    }
}
