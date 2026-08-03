use agent_desktop_core::{AdapterError, Deadline, DisplayInfo, Rect};

/// Enumerates the active monitors and reports each with bounds, the primary
/// flag, and `scale` derived from **effective** DPI - the applied value, not
/// the requested one (A10-3's carried warning: a successful scale *request* is
/// not evidence the scale *applied*).
///
/// The dev box and both CI environments have exactly one 96-DPI display
/// (A10-3), so the per-monitor code lands with single-monitor evidence; the
/// multi-monitor path is unverified until it runs on a rig with more than one
/// monitor.
pub(crate) fn list_displays_live(_deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::{LPARAM, RECT};
        use windows_sys::Win32::Graphics::Gdi::{
            EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
        };
        use windows_sys::Win32::UI::HiDpi::GetDpiForMonitor;

        let mut displays = Vec::new();
        let capture = &mut displays as *mut Vec<DisplayInfo>;
        unsafe extern "system" fn callback(
            monitor: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            lparam: LPARAM,
        ) -> i32 {
            let capture = unsafe { &mut *(lparam as *mut Vec<DisplayInfo>) };
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
                return 1;
            }
            let primary = info.dwFlags & 1 != 0;
            let mut dpi_x: u32 = 0;
            let mut dpi_y: u32 = 0;
            let effective = unsafe { GetDpiForMonitor(monitor, 0, &mut dpi_x, &mut dpi_y) };
            let scale = if effective == 0 && dpi_x > 0 {
                dpi_x as f64 / 96.0
            } else {
                1.0
            };
            capture.push(DisplayInfo {
                id: format!("monitor-{}", monitor as usize),
                bounds: Rect {
                    x: info.rcMonitor.left as f64,
                    y: info.rcMonitor.top as f64,
                    width: (info.rcMonitor.right - info.rcMonitor.left) as f64,
                    height: (info.rcMonitor.bottom - info.rcMonitor.top) as f64,
                },
                is_primary: primary,
                scale,
            });
            1
        }

        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(callback),
                capture as isize,
            );
        }
        primaries_first(&mut displays);
        Ok(displays)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = _deadline;
        Ok(Vec::new())
    }
}

/// Orders the display list with the primary first, mirroring macOS's
/// primary-first ordering (`display.rs:86-162`).
fn primaries_first(displays: &mut [DisplayInfo]) {
    displays.sort_by(|left, right| {
        right
            .is_primary
            .cmp(&left.is_primary)
            .then_with(|| left.id.cmp(&right.id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_display_orders_first() {
        let mut displays = vec![
            DisplayInfo {
                id: "monitor-2".into(),
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                is_primary: false,
                scale: 1.0,
            },
            DisplayInfo {
                id: "monitor-1".into(),
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                is_primary: true,
                scale: 1.0,
            },
        ];
        primaries_first(&mut displays);

        assert!(displays[0].is_primary);
        assert_eq!(displays[0].id, "monitor-1");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn live_listing_returns_at_least_one_primary_display() {
        crate::tree::fixture::ensure_test_apartment();
        let displays = list_displays_live(agent_desktop_core::Deadline::after(5_000).unwrap())
            .expect("live display enumeration succeeds");

        assert!(
            displays.iter().any(|display| display.is_primary),
            "exactly one primary display"
        );
        assert!(
            displays
                .iter()
                .all(|display| display.scale >= 1.0 && display.scale.is_finite()),
            "scale is rule-shaped: finite and at least 1.0"
        );
        assert!(
            displays
                .iter()
                .all(|display| display.bounds.width > 0.0 && display.bounds.height > 0.0),
            "bounds are non-degenerate"
        );
    }
}
