use crate::error::AdapterError;

/// A live handle to adapter-native connection affinity opened by
/// [`crate::adapter::SystemOps::open_session`] for a host that outlives a
/// single command (an FFI embedder, a future daemon) — the landing zone for
/// state like a Windows COM-MTA apartment thread or a Linux D-Bus
/// connection. It may hold that native connection state for as long as the
/// session stays open, but it must never hold a resolved element handle:
/// commands keep resolving elements per call from a `RefEntry` (the
/// resolve-then-release RAII boundary), and a session must not become a
/// second place stale identity can hide.
///
/// `Send + Sync` so a persistent host can hand the boxed session across
/// threads, matching every other adapter capability trait
/// (`ObservationOps`, `ActionOps`, `InputOps`, `SystemOps`).
pub trait AdapterSession: Send + Sync {
    fn close(self: Box<Self>) -> Result<(), AdapterError>;
}

#[cfg(test)]
#[path = "adapter_session_tests.rs"]
mod tests;
