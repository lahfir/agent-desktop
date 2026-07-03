/// Which CLI-level session (see [`crate::session::SessionManifest`]) a
/// long-lived adapter session should be affiliated with. Extends the
/// manifest's `id` vocabulary into the adapter layer so a persistent host
/// (an FFI embedder, a future daemon) can scope native connection affinity
/// — a Windows COM-MTA apartment thread, a Linux D-Bus connection — to the
/// same lifetime as the caller's session. `None` means no session is known;
/// `open_session` implementations remain free to open an unaffiliated
/// connection in that case.
#[derive(Debug, Clone, Default)]
pub struct SessionAffinity {
    pub session_id: Option<String>,
}
