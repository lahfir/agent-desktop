use serde::{Deserialize, Serialize};

use crate::Point;

pub const MAX_DRAG_DURATION_MS: u64 = 60_000;
pub const MAX_DRAG_DROP_DELAY_MS: u64 = 10_000;
pub const MAX_DRAG_STEPS: u64 = 4_096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragParams {
    pub from: Point,
    pub to: Point,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_delay_ms: Option<u64>,
}

impl DragParams {
    pub fn validate(&self, deadline: crate::Deadline) -> Result<(), crate::AdapterError> {
        self.from.validate()?;
        self.to.validate()?;
        let duration = self.duration_ms.unwrap_or(0);
        let drop_delay = self.drop_delay_ms.unwrap_or(0);
        if duration > MAX_DRAG_DURATION_MS || drop_delay > MAX_DRAG_DROP_DELAY_MS {
            return Err(crate::AdapterError::new(
                crate::ErrorCode::InvalidArgs,
                "Drag timing exceeds the supported maximum",
            ));
        }
        let total = duration.checked_add(drop_delay).ok_or_else(|| {
            crate::AdapterError::new(crate::ErrorCode::InvalidArgs, "Drag timing overflows")
        })?;
        let steps = duration.div_ceil(16).max(1);
        if steps > MAX_DRAG_STEPS {
            return Err(crate::AdapterError::new(
                crate::ErrorCode::InvalidArgs,
                "Drag would exceed the maximum synthesized step count",
            ));
        }
        if deadline.remaining() < std::time::Duration::from_millis(total) {
            return Err(deadline.timeout_error());
        }
        Ok(())
    }
}
