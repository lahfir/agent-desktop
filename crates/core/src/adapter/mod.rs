pub(crate) mod actions;
pub(crate) mod input;
pub(crate) mod observation;
pub(crate) mod system;
#[cfg(test)]
mod test_support;

pub(crate) use actions::ActionOps;
pub(crate) use input::InputOps;
pub(crate) use observation::ObservationOps;
pub(crate) use observation::optional_live_read;
pub(crate) use system::SystemOps;
#[cfg(test)]
pub(crate) use test_support::{
    complete_live_observation, exact_window_focus, guarded_interaction_lease, live_identity,
    observed_tree,
};

pub(crate) use crate::live_element::LiveElement;
pub(crate) use crate::native_handle::NativeHandle;
pub(crate) use crate::screenshot_target::ScreenshotTarget;
pub(crate) use crate::snapshot_surface::SnapshotSurface;
pub(crate) use crate::tree_options::TreeOptions;
pub(crate) use crate::window_filter::WindowFilter;

pub trait PlatformAdapter: ObservationOps + ActionOps + InputOps + SystemOps {}

impl<T: ObservationOps + ActionOps + InputOps + SystemOps> PlatformAdapter for T {}
