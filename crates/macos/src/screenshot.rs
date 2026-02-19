use agent_desktop_core::{adapter::ImageBuffer, adapter::ImageFormat, error::AdapterError};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::process::Command;

    pub fn capture_app(pid: i32) -> Result<ImageBuffer, AdapterError> {
        let temp = temp_path();
        let cg_id = find_cg_window_id_for_pid(pid);

        let status = if let Some(wid) = cg_id {
            Command::new("screencapture")
                .args(["-x", "-t", "png", "-l"])
                .arg(wid.to_string())
                .arg(&temp)
                .status()
        } else {
            Command::new("screencapture")
                .args(["-x", "-t", "png"])
                .arg(&temp)
                .status()
        }
        .map_err(|e| AdapterError::internal(format!("screencapture: {e}")))?;

        if !status.success() {
            return Err(AdapterError::internal("screencapture exited with error"));
        }

        read_png(&temp)
    }

    pub fn capture_screen(_idx: usize) -> Result<ImageBuffer, AdapterError> {
        let temp = temp_path();
        let status = Command::new("screencapture")
            .args(["-x", "-t", "png"])
            .arg(&temp)
            .status()
            .map_err(|e| AdapterError::internal(format!("screencapture: {e}")))?;

        if !status.success() {
            return Err(AdapterError::internal("screencapture exited with error"));
        }

        read_png(&temp)
    }

    fn temp_path() -> String {
        format!("/tmp/agent-desktop-ss-{}.png", std::process::id())
    }

    fn read_png(path: &str) -> Result<ImageBuffer, AdapterError> {
        let data = std::fs::read(path)
            .map_err(|e| AdapterError::internal(format!("read screenshot: {e}")))?;
        let _ = std::fs::remove_file(path);
        Ok(ImageBuffer { data, format: ImageFormat::Png, width: 0, height: 0 })
    }

    fn find_cg_window_id_for_pid(pid: i32) -> Option<u32> {
        use core_foundation::{
            array::CFArray,
            base::{CFType, CFTypeRef, TCFType},
            dictionary::CFDictionary,
            number::CFNumber,
            string::CFString,
        };

        extern "C" {
            fn CGWindowListCopyWindowInfo(option: u32, window_id: u32) -> CFTypeRef;
        }

        // kCGWindowListOptionOnScreenOnly (1) | kCGWindowListExcludeDesktopElements (16) = 17
        let info_ref = unsafe { CGWindowListCopyWindowInfo(17, 0) };
        if info_ref.is_null() {
            return None;
        }

        let array = unsafe { CFArray::<CFType>::wrap_under_create_rule(info_ref as _) };

        let mut best_id: Option<u32> = None;
        let mut best_area: f64 = 0.0;

        for item in array.iter() {
            let dict = unsafe {
                CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                    item.as_concrete_TypeRef() as _,
                )
            };

            let int_field = |key: &str| -> Option<i32> {
                let k = CFString::new(key);
                dict.find(&k).and_then(|v| {
                    let n = unsafe { CFNumber::wrap_under_get_rule(v.as_concrete_TypeRef() as _) };
                    n.to_i32()
                })
            };

            if int_field("kCGWindowOwnerPID") != Some(pid) {
                continue;
            }
            // Skip non-normal layers (menus, panels, overlays)
            if int_field("kCGWindowLayer").unwrap_or(99) != 0 {
                continue;
            }

            let wid = match int_field("kCGWindowNumber") {
                Some(n) => n as u32,
                None => continue,
            };

            // Pick the window with the largest area (the main window)
            let bounds_key = CFString::new("kCGWindowBounds");
            let area = if let Some(bounds_val) = dict.find(&bounds_key) {
                let bounds_dict = unsafe {
                    CFDictionary::<CFString, CFType>::wrap_under_get_rule(
                        bounds_val.as_concrete_TypeRef() as _,
                    )
                };
                let w = bounds_dict.find(&CFString::new("Width")).and_then(|v| {
                    let n =
                        unsafe { CFNumber::wrap_under_get_rule(v.as_concrete_TypeRef() as _) };
                    n.to_f64()
                });
                let h = bounds_dict.find(&CFString::new("Height")).and_then(|v| {
                    let n =
                        unsafe { CFNumber::wrap_under_get_rule(v.as_concrete_TypeRef() as _) };
                    n.to_f64()
                });
                w.unwrap_or(0.0) * h.unwrap_or(0.0)
            } else {
                0.0
            };

            if area > best_area {
                best_area = area;
                best_id = Some(wid);
            }
        }

        best_id
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn capture_app(_pid: i32) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("capture_app"))
    }

    pub fn capture_screen(_idx: usize) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("capture_screen"))
    }
}

pub use imp::{capture_app, capture_screen};
