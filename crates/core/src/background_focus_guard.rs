use serde::Serialize;

/// What the focus guard observed while it watched the frontmost application
/// during a background delivery.
///
/// `interventions` counts attempts to hand the frontmost position back to the
/// application that was frontmost before delivery after the target took it,
/// `restored` is true only when a steal happened and that application was
/// frontmost again when the guard stopped, and `max_steal_ms` is the longest
/// observed stretch during which the target was frontmost. `yielded` is true
/// when a third application (neither the user's nor the target) became
/// frontmost: the guard treats that as a deliberate switch, stops watching,
/// and never switches back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct BackgroundFocusGuard {
    pub interventions: u32,
    pub restored: bool,
    pub max_steal_ms: u64,
    pub yielded: bool,
}
