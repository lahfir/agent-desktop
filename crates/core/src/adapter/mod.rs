mod actions;
mod input;
mod observation;
mod system;

pub use actions::ActionOps;
pub use input::InputOps;
pub use observation::ObservationOps;
pub(crate) use observation::optional_live_read;
pub use system::SystemOps;

pub use crate::image_buffer::{ImageBuffer, ImageFormat};
pub use crate::live_element::LiveElement;
pub use crate::native_handle::NativeHandle;
pub use crate::screenshot_target::ScreenshotTarget;
pub use crate::snapshot_surface::SnapshotSurface;
pub use crate::tree_options::TreeOptions;
pub use crate::window_filter::WindowFilter;

pub trait PlatformAdapter: ObservationOps + ActionOps + InputOps + SystemOps {}

impl<T: ObservationOps + ActionOps + InputOps + SystemOps> PlatformAdapter for T {}
