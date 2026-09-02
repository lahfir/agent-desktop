//! The live monitor list and the refresh rate, read from the desktop.
//!
//! The refresh source is `GetDeviceCaps(VREFRESH)` on the screen DC, not
//! `EnumDisplaySettings`: the obvious call fails on this class of host and
//! leaves its frequency at zero, which a frame clock would take as an
//! infinite rate. The clamp downstream is the second guard; this is the
//! first.
//!
//! The monitor record is crate-local rather than core's `DisplayInfo`
//! because the overlay needs each monitor's **work area** to place a label
//! and to pick a resting point, and `DisplayInfo` carries none. Extending it
//! would change `list-displays` output on both platforms.

#[cfg(target_os = "windows")]
pub(crate) use imp::{monitors, refresh_hz};

#[cfg(not(target_os = "windows"))]
pub(crate) fn monitors() -> Vec<super::monitors::OverlayMonitor> {
    Vec::new()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn refresh_hz() -> u32 {
    0
}

#[cfg(target_os = "windows")]
mod imp {
    use crate::system::cursor_overlay::monitors::OverlayMonitor;
    use agent_desktop_core::Rect;
    use windows_sys::Win32::Foundation::{LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetDC, GetDeviceCaps, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
        ReleaseDC, VREFRESH,
    };
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows_sys::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

    const USER_DEFAULT_SCREEN_DPI: f64 = 96.0;

    pub(crate) fn refresh_hz() -> u32 {
        let screen = unsafe { GetDC(std::ptr::null_mut()) };
        let hz = unsafe { GetDeviceCaps(screen, VREFRESH as i32) };
        unsafe { ReleaseDC(std::ptr::null_mut(), screen) };
        hz.max(0) as u32
    }

    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _dc: HDC,
        _rect: *mut RECT,
        payload: LPARAM,
    ) -> i32 {
        let collected = unsafe { &mut *(payload as *mut Vec<OverlayMonitor>) };
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
            return 1;
        }
        let mut dpi_x = 0u32;
        let mut dpi_y = 0u32;
        let scale =
            if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) } == 0
                && dpi_x > 0
            {
                f64::from(dpi_x) / USER_DEFAULT_SCREEN_DPI
            } else {
                1.0
            };
        collected.push(OverlayMonitor {
            bounds: rect_of(&info.rcMonitor),
            work_area: rect_of(&info.rcWork),
            scale,
            is_primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
        1
    }

    fn rect_of(rect: &RECT) -> Rect {
        Rect {
            x: f64::from(rect.left),
            y: f64::from(rect.top),
            width: f64::from(rect.right - rect.left),
            height: f64::from(rect.bottom - rect.top),
        }
    }

    pub(crate) fn monitors() -> Vec<OverlayMonitor> {
        let mut collected: Vec<OverlayMonitor> = Vec::new();
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(collect),
                &mut collected as *mut Vec<OverlayMonitor> as LPARAM,
            );
        }
        collected
    }
}
