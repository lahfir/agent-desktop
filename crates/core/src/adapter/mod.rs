mod actions;
mod input;
mod observation;
mod system;
#[cfg(test)]
mod test_support;

pub use actions::ActionOps;
pub use input::InputOps;
pub use observation::ObservationOps;
pub(crate) use observation::optional_live_read;
pub use system::SystemOps;
#[cfg(test)]
pub(crate) use test_support::{
    complete_live_observation, guarded_interaction_lease, live_identity, observed_tree,
};

pub use crate::live_element::LiveElement;
pub use crate::native_handle::NativeHandle;
pub use crate::screenshot_target::ScreenshotTarget;
pub use crate::snapshot_surface::SnapshotSurface;
pub use crate::tree_options::TreeOptions;
pub use crate::window_filter::WindowFilter;

pub trait PlatformAdapter: ObservationOps + ActionOps + InputOps + SystemOps {}

impl<T: ObservationOps + ActionOps + InputOps + SystemOps> PlatformAdapter for T {}
