//! Legacy window capture via `PrintWindow` and a pump-liveness pre-probe.
//!
//! Call order owned by the caller: corroborate window identity first, then
//! invoke [`capture_window`]. This module refuses a destroyed handle before
//! the pump probe so a gone window is not mislabelled `APP_UNRESPONSIVE`,
//! then probes responsiveness, then issues `PrintWindow`.

#![allow(dead_code)]

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, ImageBuffer, ImageFormat};

use super::permissions::ensure_budget;
use super::png_codec::encode_bgra_to_png;
use super::process_state::hresult_from_win32;
use super::window_enum::{WindowHandle, window_is_responsive};

/// Undocumented `PrintWindow` flag (Windows 8.1+) that asks the target for a
/// full composited frame rather than a client-only `WM_PRINT` pass. Not in
/// the official parameter table — only `PW_CLIENTONLY` is documented there.
/// A22-2 measured non-black frames with this flag on this project's GDI
/// fixture stacks.
const PW_RENDERFULLCONTENT: u32 = 0x0000_0002;

// Linked directly from user32.dll. The windows-sys binding for this symbol
// lives behind Win32_Storage_Xps, which this crate does not take; declaring
// the import locally keeps the legacy capture path free of that feature the
// same way PW_RENDERFULLCONTENT is declared locally.
#[link(name = "user32")]
unsafe extern "system" {
    fn PrintWindow(
        hwnd: WindowHandle,
        hdcblt: windows_sys::Win32::Graphics::Gdi::HDC,
        nflags: u32,
    ) -> i32;
}

const MINIMIZED_ORIGIN_SENTINEL: i32 = -32_000;

/// Captures `handle` into a PNG [`ImageBuffer`] using GDI `PrintWindow`.
///
/// Requires the caller to have already verified window identity. Performs the
/// pump-liveness pre-probe immediately before the native capture call.
pub(crate) fn capture_window(
    handle: WindowHandle,
    scale_factor: f64,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    ensure_budget(deadline)?;
    ensure_handle_alive(handle)?;
    ensure_window_is_pumping(handle)?;
    let (width, height) = window_capture_size(handle)?;
    let pixels = print_window_bgra(handle, width, height)?;
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| AdapterError::internal("window capture stride overflowed"))?;
    let png = encode_bgra_to_png(&pixels, width, height, stride, deadline)?;
    Ok(ImageBuffer {
        data: png,
        format: ImageFormat::Png,
        width,
        height,
        scale_factor,
    })
}

fn ensure_handle_alive(handle: WindowHandle) -> Result<(), AdapterError> {
    if crate::tree::automation::window_exists(handle as isize) {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::WindowNotFound,
        "The window handle no longer addresses a live window",
    )
    .with_suggestion("Run 'list-windows' to refresh window identifiers, then retry.")
    .with_platform_detail(format!(
        "HWND 0x{:X} failed the pre-capture existence check",
        handle as usize
    )))
}

fn ensure_window_is_pumping(handle: WindowHandle) -> Result<(), AdapterError> {
    if window_is_responsive(handle) {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Target window is not processing messages, so PrintWindow would block",
    )
    .with_suggestion("Wait for the application to recover, then retry the screenshot"))
}

fn window_capture_size(handle: WindowHandle) -> Result<(u32, u32), AdapterError> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic};

    let mut rect = RECT::default();
    if unsafe { GetWindowRect(handle, &mut rect) } == 0 {
        return Err(win32_last_error(
            "GetWindowRect failed before window capture",
        ));
    }
    if unsafe { IsIconic(handle) } != 0
        || rect.left <= MINIMIZED_ORIGIN_SENTINEL
        || rect.top <= MINIMIZED_ORIGIN_SENTINEL
    {
        return Err(
            AdapterError::new(ErrorCode::InvalidArgs, "Cannot capture a minimized window")
                .with_suggestion("Restore the window, then retry the screenshot"),
        );
    }
    let width = rect.right.saturating_sub(rect.left);
    let height = rect.bottom.saturating_sub(rect.top);
    if width <= 0 || height <= 0 {
        return Err(
            AdapterError::new(ErrorCode::InvalidArgs, "Cannot capture a zero-area window")
                .with_suggestion("Restore or resize the window, then retry the screenshot"),
        );
    }
    Ok((width as u32, height as u32))
}

fn print_window_bgra(
    handle: WindowHandle,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, AdapterError> {
    #[cfg(test)]
    if fail_after_alloc::is_active() {
        // Allocate then fail so the RAII balance test covers Drop on the
        // error path without depending on PrintWindow succeeding.
        let _surface = CaptureSurface::create(width as i32, height as i32)?;
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "forced capture failure after GDI allocation",
        ));
    }
    // Prefer the undocumented full-content flag (A22-2); when that path
    // yields an empty frame — common for WS_POPUP windows that do not
    // participate in DWM redirection — fall back to classic WM_PRINT so the
    // window's WM_PRINTCLIENT handler can paint into the DC.
    let full = capture_with_flags(handle, width, height, PW_RENDERFULLCONTENT)?;
    if !frame_appears_empty(&full) {
        return Ok(full);
    }
    capture_with_flags(handle, width, height, 0)
}

fn capture_with_flags(
    handle: WindowHandle,
    width: u32,
    height: u32,
    flags: u32,
) -> Result<Vec<u8>, AdapterError> {
    let surface = CaptureSurface::create(width as i32, height as i32)?;
    if unsafe { PrintWindow(handle, surface.memory_dc, flags) } == 0 {
        return Err(win32_last_error("PrintWindow failed"));
    }
    surface.read_bgra()
}

fn frame_appears_empty(pixels: &[u8]) -> bool {
    !pixels
        .chunks_exact(4)
        .any(|px| px[0] != 0 || px[1] != 0 || px[2] != 0)
}

fn win32_last_error(message: &str) -> AdapterError {
    let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    let hresult = hresult_from_win32(error);
    let record = super::hresult::hresult_record(hresult);
    let mut err = AdapterError::new(record.code, message)
        .with_platform_detail(super::hresult::com_hresult_detail(hresult));
    if let Some(suggestion) = record.suggestion {
        err = err.with_suggestion(suggestion);
    }
    err
}

/// Device-compatible bitmap surface matching the A22-2 probe path:
/// `CreateCompatibleBitmap` + `PrintWindow` + `GetDIBits`. A DIB-section
/// target can stay black under `PW_RENDERFULLCONTENT` even when the same
/// call into a compatible bitmap yields the painted frame.
struct CaptureSurface {
    screen_dc: windows_sys::Win32::Graphics::Gdi::HDC,
    memory_dc: windows_sys::Win32::Graphics::Gdi::HDC,
    bitmap: *mut core::ffi::c_void,
    previous: *mut core::ffi::c_void,
    width: i32,
    height: i32,
}

impl CaptureSurface {
    fn create(width: i32, height: i32) -> Result<Self, AdapterError> {
        use windows_sys::Win32::Graphics::Gdi::{
            CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, GetDC, ReleaseDC, SelectObject,
        };

        let screen_dc = unsafe { GetDC(std::ptr::null_mut()) };
        if screen_dc.is_null() {
            return Err(win32_last_error("GetDC failed for window capture"));
        }
        gdi_balance::acquire();
        let memory_dc = unsafe { CreateCompatibleDC(screen_dc) };
        if memory_dc.is_null() {
            unsafe { ReleaseDC(std::ptr::null_mut(), screen_dc) };
            gdi_balance::release();
            return Err(win32_last_error(
                "CreateCompatibleDC failed for window capture",
            ));
        }
        gdi_balance::acquire();
        let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };
        if bitmap.is_null() {
            unsafe {
                DeleteDC(memory_dc);
                ReleaseDC(std::ptr::null_mut(), screen_dc);
            }
            gdi_balance::release();
            gdi_balance::release();
            return Err(win32_last_error(
                "CreateCompatibleBitmap failed for window capture",
            ));
        }
        gdi_balance::acquire();
        let previous = unsafe { SelectObject(memory_dc, bitmap) };
        Ok(Self {
            screen_dc,
            memory_dc,
            bitmap,
            previous,
            width,
            height,
        })
    }

    fn read_bgra(self) -> Result<Vec<u8>, AdapterError> {
        use windows_sys::Win32::Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, GetDIBits,
        };

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: self.width,
                biHeight: -self.height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (self.width * self.height * 4) as usize];
        let copied = unsafe {
            GetDIBits(
                self.memory_dc,
                self.bitmap,
                0,
                self.height as u32,
                pixels.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            )
        };
        if copied == 0 {
            return Err(win32_last_error("GetDIBits failed after PrintWindow"));
        }
        Ok(pixels)
    }
}

impl Drop for CaptureSurface {
    fn drop(&mut self) {
        use windows_sys::Win32::Graphics::Gdi::{DeleteDC, DeleteObject, ReleaseDC, SelectObject};
        unsafe {
            SelectObject(self.memory_dc, self.previous);
            DeleteObject(self.bitmap);
            gdi_balance::release();
            DeleteDC(self.memory_dc);
            gdi_balance::release();
            ReleaseDC(std::ptr::null_mut(), self.screen_dc);
            gdi_balance::release();
        }
    }
}

mod gdi_balance {
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<i32> = const { Cell::new(0) };
    }

    pub(super) fn acquire() {
        LIVE.with(|cell| cell.set(cell.get() + 1));
    }

    pub(super) fn release() {
        LIVE.with(|cell| cell.set(cell.get() - 1));
    }

    #[cfg(test)]
    pub(super) fn live() -> i32 {
        LIVE.with(Cell::get)
    }

    #[cfg(test)]
    pub(super) fn reset() {
        LIVE.with(|cell| cell.set(0));
    }
}

#[cfg(test)]
pub(super) mod fail_after_alloc {
    use std::cell::Cell;

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn is_active() -> bool {
        ACTIVE.with(Cell::get)
    }

    pub(super) fn with<R>(run: impl FnOnce() -> R) -> R {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                ACTIVE.with(|cell| cell.set(false));
            }
        }
        ACTIVE.with(|cell| cell.set(true));
        let _reset = Reset;
        run()
    }
}

#[cfg(test)]
#[path = "capture_window_tests.rs"]
mod tests;
