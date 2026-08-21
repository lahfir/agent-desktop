use crate::{AdapterError, CursorOverlayConfig, InteractionPolicy, SignalBaseline};

use super::WaitSelector;

#[derive(Debug, Clone, Default)]
pub(super) struct CommandOptions {
    pub interaction_policy: InteractionPolicy,
    pub wait_selector: Option<WaitSelector>,
    pub event_baseline: Option<Result<SignalBaseline, AdapterError>>,
    pub cursor_overlay: CursorOverlayConfig,
}

impl CommandOptions {
    pub fn for_batch(&self, cursor_overlay: CursorOverlayConfig) -> Self {
        Self {
            interaction_policy: self.interaction_policy,
            wait_selector: None,
            event_baseline: None,
            cursor_overlay,
        }
    }
}
