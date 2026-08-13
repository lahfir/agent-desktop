//! Shared GDI screen/memory device-context pairing for the two Legacy
//! capture backends.
//!
//! `capture_window.rs` selects a compatible bitmap into the memory DC and
//! reads it back with `GetDIBits`; `capture_display.rs` selects a DIB
//! section and reads its mapped bits directly. Those two bitmap shapes stay
//! distinct on purpose — folding them into one type would fit neither read
//! path. What both surfaces do identically is open a screen DC, pair it with
//! a compatible memory DC, and tear the pair down in the same order; that
//! pairing, the GDI-object balance counter, and the `GetLastError` mapping
//! live here once instead of twice.

use agent_desktop_core::AdapterError;
use windows_sys::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, HDC};

use super::hresult::{com_hresult_detail, hresult_record};
use super::process_state::hresult_from_win32;

pub(super) fn top_down_bgra_bitmap_info(width: i32, height: i32) -> BITMAPINFO {
    BITMAPINFO {
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
    }
}

pub(super) fn win32_last_error(message: &str) -> AdapterError {
    let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    let hresult = hresult_from_win32(error);
    let record = hresult_record(hresult);
    let mut err =
        AdapterError::new(record.code, message).with_platform_detail(com_hresult_detail(hresult));
    if let Some(suggestion) = record.suggestion {
        err = err.with_suggestion(suggestion);
    }
    err
}

/// The screen DC and its paired compatible memory DC, released together.
///
/// Owns exactly the two GDI objects every Legacy capture surface opens
/// before selecting its own bitmap into `memory_dc`; the bitmap stays with
/// the caller, since a compatible bitmap and a DIB section are read back
/// through different APIs.
pub(super) struct GdiDcPair {
    pub(super) screen_dc: HDC,
    pub(super) memory_dc: HDC,
}

impl GdiDcPair {
    pub(super) fn create(context: &str) -> Result<Self, AdapterError> {
        use windows_sys::Win32::Graphics::Gdi::{CreateCompatibleDC, GetDC, ReleaseDC};

        let screen_dc = unsafe { GetDC(std::ptr::null_mut()) };
        if screen_dc.is_null() {
            return Err(win32_last_error(&format!("GetDC failed for {context}")));
        }
        gdi_balance::acquire();
        let memory_dc = unsafe { CreateCompatibleDC(screen_dc) };
        if memory_dc.is_null() {
            unsafe { ReleaseDC(std::ptr::null_mut(), screen_dc) };
            gdi_balance::release();
            return Err(win32_last_error(&format!(
                "CreateCompatibleDC failed for {context}"
            )));
        }
        gdi_balance::acquire();
        Ok(Self {
            screen_dc,
            memory_dc,
        })
    }
}

impl Drop for GdiDcPair {
    fn drop(&mut self) {
        use windows_sys::Win32::Graphics::Gdi::{DeleteDC, ReleaseDC};
        unsafe {
            DeleteDC(self.memory_dc);
            gdi_balance::release();
            ReleaseDC(std::ptr::null_mut(), self.screen_dc);
            gdi_balance::release();
        }
    }
}

/// Restores the memory DC's prior GDI selection, deletes `bitmap`, and decrements
/// the balance counter acquired when the bitmap was selected.
pub(super) fn restore_selected_bitmap(
    memory_dc: HDC,
    previous: *mut core::ffi::c_void,
    bitmap: *mut core::ffi::c_void,
) {
    use windows_sys::Win32::Graphics::Gdi::{DeleteObject, SelectObject};
    unsafe {
        SelectObject(memory_dc, previous);
        DeleteObject(bitmap);
        gdi_balance::release();
    }
}

pub(crate) mod gdi_balance {
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<i32> = const { Cell::new(0) };
    }

    pub(crate) fn acquire() {
        LIVE.with(|cell| cell.set(cell.get() + 1));
    }

    pub(crate) fn release() {
        LIVE.with(|cell| cell.set(cell.get() - 1));
    }

    #[cfg(test)]
    pub(crate) fn live() -> i32 {
        LIVE.with(Cell::get)
    }

    #[cfg(test)]
    pub(crate) fn reset() {
        LIVE.with(|cell| cell.set(0));
    }
}
