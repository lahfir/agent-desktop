use agent_desktop_core::{
    display_info::DisplayInfo,
    error::{AdapterError, ErrorCode},
    node::Rect,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_graphics::display::CGDisplay;

    pub fn list_displays_impl() -> Result<Vec<DisplayInfo>, AdapterError> {
        let displays = CGDisplay::active_displays()
            .map_err(|_| AdapterError::internal("Failed to enumerate active displays"))?;
        let main_id = CGDisplay::main().id;
        let mut infos: Vec<DisplayInfo> = displays
            .iter()
            .map(|display_id| display_info(&CGDisplay::new(*display_id), main_id))
            .collect();
        infos.sort_by(|left, right| {
            right
                .is_primary
                .cmp(&left.is_primary)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(infos)
    }

    pub fn display_at(index: usize) -> Result<DisplayInfo, AdapterError> {
        let displays = list_displays_impl()?;
        displays
            .into_iter()
            .nth(index)
            .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "display index out of range"))
    }

    fn display_info(display: &CGDisplay, main_id: u32) -> DisplayInfo {
        let bounds = display.bounds();
        let point_width = bounds.size.width;
        let pixel_width = display.pixels_wide() as f64;
        let scale = if point_width > 0.0 {
            pixel_width / point_width
        } else {
            1.0
        };
        DisplayInfo {
            id: display.id.to_string(),
            bounds: Rect {
                x: bounds.origin.x,
                y: bounds.origin.y,
                width: bounds.size.width,
                height: bounds.size.height,
            },
            is_primary: display.id == main_id,
            scale,
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn list_displays_impl() -> Result<Vec<DisplayInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_displays"))
    }

    pub fn display_at(_index: usize) -> Result<DisplayInfo, AdapterError> {
        Err(AdapterError::not_supported("list_displays"))
    }
}

pub use imp::{display_at, list_displays_impl};
