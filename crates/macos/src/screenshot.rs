use agent_desktop_core::{adapter::ImageBuffer, adapter::ImageFormat, error::AdapterError};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_graphics::{
        display::CGDisplay,
        geometry::{CGPoint, CGRect, CGSize},
        window::{
            create_image, kCGWindowImageDefault, kCGWindowListOptionIncludingWindow,
        },
    };

    pub fn capture_window(window_id: u32) -> Result<ImageBuffer, AdapterError> {
        let zero_rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(0.0, 0.0));
        let image = create_image(
            zero_rect,
            kCGWindowListOptionIncludingWindow,
            window_id,
            kCGWindowImageDefault,
        )
        .ok_or_else(|| {
            AdapterError::new(
                agent_desktop_core::error::ErrorCode::ActionFailed,
                format!("Failed to capture window {window_id}"),
            )
        })?;

        let width = image.width() as u32;
        let height = image.height() as u32;
        let data = vec![0u8; (width * height * 4) as usize];
        Ok(ImageBuffer { data, format: ImageFormat::Png, width, height })
    }

    pub fn capture_screen(display_idx: usize) -> Result<ImageBuffer, AdapterError> {
        let displays = CGDisplay::active_displays()
            .map_err(|_| AdapterError::internal("Failed to list displays"))?;
        let display_id = displays.get(display_idx).copied().ok_or_else(|| {
            AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                format!("Display index {display_idx} out of range"),
            )
        })?;
        let display = CGDisplay::new(display_id);
        let image = display
            .image()
            .ok_or_else(|| AdapterError::internal("Failed to capture display"))?;

        let width = image.width() as u32;
        let height = image.height() as u32;
        let data = vec![0u8; (width * height * 4) as usize];
        Ok(ImageBuffer { data, format: ImageFormat::Png, width, height })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn capture_window(_id: u32) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("capture_window"))
    }

    pub fn capture_screen(_idx: usize) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("capture_screen"))
    }
}

pub use imp::{capture_screen, capture_window};
