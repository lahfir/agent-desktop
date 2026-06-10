use crate::types::point::AdPoint;
use agent_desktop_core::action::{DragParams as CoreDragParams, Point as CorePoint};

#[repr(C)]
pub struct AdDragParams {
    pub from: AdPoint,
    pub to: AdPoint,
    pub duration_ms: u64,
    pub drop_delay_ms: u64,
}

impl AdDragParams {
    /// Converts the C drag params into the core type. `duration_ms` and
    /// `drop_delay_ms` use `0` as the "adapter default" sentinel because the
    /// C ABI has no `Option`; any non-zero value is passed through.
    pub(crate) fn to_core(&self) -> CoreDragParams {
        CoreDragParams {
            from: CorePoint {
                x: self.from.x,
                y: self.from.y,
            },
            to: CorePoint {
                x: self.to.x,
                y: self.to.y,
            },
            duration_ms: zero_as_default(self.duration_ms),
            drop_delay_ms: zero_as_default(self.drop_delay_ms),
        }
    }
}

fn zero_as_default(value: u64) -> Option<u64> {
    (value != 0).then_some(value)
}
