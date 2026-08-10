//! Legacy display capture via `BitBlt` from the screen DC.
//!
//! Public index 0 is the primary display — the `FullScreen` target maps here,
//! never to the virtual-screen span. Monitor origins may be negative on
//! multi-monitor desktops; the blit source uses those coordinates as-is.

#![allow(dead_code)]

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, ImageBuffer, ImageFormat, Rect};

use super::display::display_at;
use super::permissions::ensure_budget;
use super::png_codec::encode_bgra_to_png;
use super::process_state::hresult_from_win32;

/// Captures the display at the public enumeration index into a PNG buffer.
///
/// Index 0 is the primary (`FullScreen`). Dimensions and scale come from the
/// same enumeration the blit targets — never from literals.
pub(crate) fn capture_display_at(
    index: usize,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    ensure_budget(deadline)?;
    let display = display_at(index, deadline)?;
    capture_display_bounds(display.bounds, display.scale, deadline)
}

/// Captures the primary display — the cross-platform `FullScreen` mapping.
pub(crate) fn capture_fullscreen(deadline: Deadline) -> Result<ImageBuffer, AdapterError> {
    capture_display_at(0, deadline)
}

pub(super) fn capture_display_bounds(
    bounds: Rect,
    scale_factor: f64,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    ensure_budget(deadline)?;
    let (width, height, origin_x, origin_y) = display_capture_geometry(bounds)?;
    let pixels = bitblt_screen_bgra(origin_x, origin_y, width, height)?;
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| AdapterError::internal("display capture stride overflowed"))?;
    let png = encode_bgra_to_png(&pixels, width, height, stride, deadline)?;
    Ok(ImageBuffer {
        data: png,
        format: ImageFormat::Png,
        width,
        height,
        scale_factor,
    })
}

pub(super) fn display_capture_geometry(bounds: Rect) -> Result<(u32, u32, i32, i32), AdapterError> {
    if !bounds.width.is_finite()
        || !bounds.height.is_finite()
        || !bounds.x.is_finite()
        || !bounds.y.is_finite()
    {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Display bounds must be finite",
        ));
    }
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Cannot capture a zero-area display",
        ));
    }
    let width = bounds.width as i32;
    let height = bounds.height as i32;
    if width <= 0 || height <= 0 {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Cannot capture a zero-area display",
        ));
    }
    Ok((
        width as u32,
        height as u32,
        bounds.x as i32,
        bounds.y as i32,
    ))
}

fn bitblt_screen_bgra(
    origin_x: i32,
    origin_y: i32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, AdapterError> {
    let surface = DibSection::create(width as i32, height as i32)?;
    #[cfg(test)]
    if fail_after_alloc::is_active() {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "forced capture failure after GDI allocation",
        ));
    }
    use windows_sys::Win32::Graphics::Gdi::{BitBlt, SRCCOPY};
    if unsafe {
        BitBlt(
            surface.memory_dc,
            0,
            0,
            width as i32,
            height as i32,
            surface.screen_dc,
            origin_x,
            origin_y,
            SRCCOPY,
        )
    } == 0
    {
        return Err(win32_last_error("BitBlt failed for display capture"));
    }
    Ok(surface.into_bgra())
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

struct DibSection {
    screen_dc: windows_sys::Win32::Graphics::Gdi::HDC,
    memory_dc: windows_sys::Win32::Graphics::Gdi::HDC,
    bitmap: *mut core::ffi::c_void,
    previous: *mut core::ffi::c_void,
    bits: *mut u8,
    width: i32,
    height: i32,
}

impl DibSection {
    fn create(width: i32, height: i32) -> Result<Self, AdapterError> {
        use windows_sys::Win32::Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, GetDC, ReleaseDC, SelectObject,
        };

        let screen_dc = unsafe { GetDC(std::ptr::null_mut()) };
        if screen_dc.is_null() {
            return Err(win32_last_error("GetDC failed for display capture"));
        }
        gdi_balance::acquire();
        let memory_dc = unsafe { CreateCompatibleDC(screen_dc) };
        if memory_dc.is_null() {
            unsafe { ReleaseDC(std::ptr::null_mut(), screen_dc) };
            gdi_balance::release();
            return Err(win32_last_error(
                "CreateCompatibleDC failed for display capture",
            ));
        }
        gdi_balance::acquire();
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits_ptr = std::ptr::null_mut();
        let bitmap = unsafe {
            CreateDIBSection(
                memory_dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                std::ptr::null_mut(),
                0,
            )
        };
        if bitmap.is_null() || bits_ptr.is_null() {
            unsafe {
                DeleteDC(memory_dc);
                ReleaseDC(std::ptr::null_mut(), screen_dc);
            }
            gdi_balance::release();
            gdi_balance::release();
            return Err(win32_last_error(
                "CreateDIBSection failed for display capture",
            ));
        }
        gdi_balance::acquire();
        let previous = unsafe { SelectObject(memory_dc, bitmap) };
        Ok(Self {
            screen_dc,
            memory_dc,
            bitmap,
            previous,
            bits: bits_ptr.cast(),
            width,
            height,
        })
    }

    fn into_bgra(self) -> Vec<u8> {
        let stride = (self.width * 4) as usize;
        let bytes = stride * self.height as usize;
        let mut pixels = vec![0u8; bytes];
        unsafe {
            std::ptr::copy_nonoverlapping(self.bits, pixels.as_mut_ptr(), bytes);
        }
        pixels
    }
}

impl Drop for DibSection {
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
#[path = "capture_display_tests.rs"]
mod tests;
