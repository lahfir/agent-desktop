pub(super) struct ResolveReadContext {
    pub(super) stats: agent_desktop_core::LocatorStats,
    pub(super) usage: crate::tree::observation_usage::ObservationUsage,
    pub(super) deadline: std::time::Instant,
}

impl ResolveReadContext {
    pub(super) fn new(deadline: std::time::Instant) -> Self {
        Self {
            stats: agent_desktop_core::LocatorStats::default(),
            usage: crate::tree::observation_usage::ObservationUsage::new(
                agent_desktop_core::ObservationBudget::default(),
            ),
            deadline,
        }
    }
}
