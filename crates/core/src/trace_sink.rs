//! Native trace persistence selected by a command host, before commands run.
//!
//! This Rust interface stays inside the engine/module. Transport adapters must
//! use their public wire contracts across dynamic-library or process boundaries.

use crate::{AppError, Deadline};
use serde_json::Value;

/// The artifact category whose aggregate budget a sink must enforce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceArtifactKind {
    /// A screenshot PNG, counted against both bytes and screenshot count.
    Screenshot,
    /// A serialized reference map, counted against reference-map bytes.
    RefMap,
}

/// Aggregate limits for one trace session, including concurrent/reopened handles.
#[derive(Debug, Clone, Copy)]
pub struct TraceArtifactLimits {
    /// Maximum total screenshot bytes.
    pub screenshot_bytes: u64,
    /// Maximum screenshot count.
    pub screenshot_count: u32,
    /// Maximum total reference-map bytes.
    pub refmap_bytes: u64,
}

/// One bounded artifact. Names identify artifacts within a bound trace, not paths.
#[derive(Debug)]
pub struct TraceArtifact<'a> {
    /// Budget category.
    pub kind: TraceArtifactKind,
    /// Producer-generated stable artifact name.
    pub name: &'a str,
    /// Artifact content; the sink must not retain this borrowed slice.
    pub bytes: &'a [u8],
    /// Deadline for admission, upload and acknowledgement.
    pub deadline: Deadline,
}

/// An acknowledged native trace sink capturing host-issued authority.
///
/// Implementations must keep scope/generation authority inside the sink rather
/// than derive it from event fields or session IDs. Append receives the same
/// sanitized, bounded event envelope as the local file writer. Success means
/// the event is committed; failure may be ambiguous and must not trigger an
/// unqualified retry of either the trace or its associated desktop action.
///
/// Methods are synchronous. An async transport adapter must execute them on a
/// blocking worker, schedule work onto an independently driven runtime, and
/// bound acknowledgement waits. It must reject use from an executor thread
/// where waiting would block its own transport; do not call current-thread
/// runtime block_on from here. This core crate does not install a runtime.
/// Append must not re-enter the same bound trace: emission is serialized across
/// its clones while the acknowledgement is pending.
///
/// Artifact storage must atomically enforce the supplied aggregate limits for
/// the bound session across concurrent/reopened handles, deduplicate reference
/// maps by name, and return a bounded opaque reference only after commit.
/// Failure never authorizes a filesystem fallback. Artifact/event commits
/// are separate: implementations must document cleanup of unreferenced blobs.
pub trait TraceSink: Send + Sync {
    /// Commit one prepared event without logging its contents on error.
    /// Enforce the cumulative serialized JSONL byte budget atomically across
    /// concurrent/reopened handles, and return by the supplied deadline.
    fn append(&self, event: &Value, deadline: Deadline, byte_budget: u64) -> Result<(), AppError>;

    /// Commit an artifact, enforcing the supplied persistent aggregate limits.
    fn store_artifact(
        &self,
        _artifact: TraceArtifact<'_>,
        _limits: TraceArtifactLimits,
    ) -> Result<String, AppError> {
        Err(AppError::invalid_input(
            "The selected trace sink does not support artifacts",
        ))
    }
}
