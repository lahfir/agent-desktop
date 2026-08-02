use agent_desktop_core::{AdapterError, Rect};

/// A top-level window handle, typed for the compiling platform.
///
/// `windows-sys`'s `HWND` is a raw pointer that only exists on Windows; the
/// crate still must compile on the Linux cross-check lane, so the system
/// modules name this alias in signatures and keep the Windows-only calls
/// behind `#[cfg]`. The non-Windows alias stays a raw pointer so
/// `std::ptr::null_mut()` and pointer casts type-check identically.
#[cfg(target_os = "windows")]
pub(crate) type WindowHandle = windows_sys::Win32::Foundation::HWND;
#[cfg(not(target_os = "windows"))]
pub(crate) type WindowHandle = *mut core::ffi::c_void;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Dwm::DWMWA_CLOAKED;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetWindowLongW, GetWindowRect, IsIconic, IsWindowVisible,
    WS_EX_TOOLWINDOW,
};

/// A top-level window as the enumeration pass records it.
///
/// Identity-bearing handle plus the geometry, visibility and ex-style facts
/// the census filter judges - every field read once, off the same HWND, so the
/// filter can cite its own evidence per criterion (A16-1).
#[derive(Debug, Clone, Copy)]
pub(crate) struct EnumeratedWindow {
    pub(crate) handle: WindowHandle,
    pub(crate) visible: bool,
    pub(crate) iconic: bool,
    pub(crate) cloaked: bool,
    pub(crate) tool: bool,
    pub(crate) rect: Rect,
}

impl EnumeratedWindow {
    pub(crate) fn is_zero_sized(&self) -> bool {
        self.rect.width <= 0.0 || self.rect.height <= 0.0
    }
}

/// Enumerates every top-level window on the calling desktop.
///
/// The closure receives each window and stops when it returns `false`, the
/// documented `EnumWindows` contract. `EnumWindows` invokes the callback
/// synchronously on the calling thread, so the visitor is passed by raw
/// pointer through the callback's `lparam` and never crosses threads; the
/// reference is valid for the entire synchronous call.
#[cfg(target_os = "windows")]
pub(crate) fn enumerate_top_level(
    visit: impl FnMut(EnumeratedWindow) -> bool,
) -> Result<(), AdapterError> {
    unsafe extern "system" fn callback(window: HWND, lparam: isize) -> i32 {
        let visit = unsafe { &mut *(lparam as *mut Box<dyn FnMut(EnumeratedWindow) -> bool>) };
        let keep_going = visit(EnumeratedWindow {
            handle: window,
            visible: unsafe { IsWindowVisible(window) != 0 },
            iconic: unsafe { IsIconic(window) != 0 },
            cloaked: is_cloaked(window),
            tool: is_tool_window(window),
            rect: window_rect(window),
        });
        i32::from(keep_going)
    }

    let mut visit: Box<dyn FnMut(EnumeratedWindow) -> bool> = Box::new(visit);
    let parameter = (&mut visit as *mut Box<dyn FnMut(EnumeratedWindow) -> bool>) as isize;
    unsafe { EnumWindows(Some(callback), parameter) };
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn enumerate_top_level(
    _visit: impl FnMut(EnumeratedWindow) -> bool,
) -> Result<(), AdapterError> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_cloaked(window: HWND) -> bool {
    let mut cloaked: u32 = 0;
    let succeeded = unsafe {
        windows_sys::Win32::Graphics::Dwm::DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED as u32,
            (&mut cloaked as *mut u32).cast(),
            core::mem::size_of::<u32>() as u32,
        )
    } == 0;
    succeeded && cloaked != 0
}

#[cfg(target_os = "windows")]
fn is_tool_window(window: HWND) -> bool {
    let ex_style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) };
    (ex_style & WS_EX_TOOLWINDOW as i32) != 0
}

#[cfg(target_os = "windows")]
fn window_rect(window: HWND) -> Rect {
    let mut rect = windows_sys::Win32::Foundation::RECT::default();
    if unsafe { GetWindowRect(window, &mut rect) } == 0 {
        return Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
    }
    Rect {
        x: rect.left as f64,
        y: rect.top as f64,
        width: (rect.right - rect.left) as f64,
        height: (rect.bottom - rect.top) as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumeration_calls_back_for_every_window_and_stops_on_false() {
        let mut visited = Vec::new();
        let mut first = true;

        enumerate_top_level(|window| {
            visited.push(window.handle);
            if first {
                first = false;
                false
            } else {
                true
            }
        })
        .expect("enumeration succeeds");

        assert_eq!(
            visited.len(),
            1,
            "the callback returned false after the first window"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn live_enumeration_observes_the_shell_without_crashing() {
        let mut visible = 0usize;
        let mut total = 0usize;
        enumerate_top_level(|window| {
            total += 1;
            if window.visible && !window.is_zero_sized() {
                visible += 1;
            }
            true
        })
        .expect("live enumeration succeeds");

        assert!(total > 0, "a desktop has at least one top-level window");
        assert!(visible > 0, "the shell is visible");
    }
}
