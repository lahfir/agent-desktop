//! Win32 window creation and message pump for the 2.2 capability probe.
//!
//! Split from `probe.rs` to keep both files inside the repository's 400-line
//! source cap.

use std::ffi::c_void;
use std::sync::mpsc::Sender;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowRect, GetWindowTextW,
    IDC_ARROW, IsWindowVisible, LoadCursorW, MSG, PostQuitMessage, RegisterClassExW,
    SW_SHOWNOACTIVATE, SetWindowTextW, ShowWindow, TranslateMessage, WM_DESTROY, WNDCLASSEXW,
    WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

const ES_PASSWORD: u32 = 0x0020;
const CONTROL_BORDER: u32 = 0x0080_0000;

pub struct WindowGeometry {
    pub visible: bool,
    pub width: i32,
    pub height: i32,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Writes the marker into the password control and proves it is there by
/// reading it back off the control.
///
/// The read-back is what makes the leak report a measurement. Every property
/// the report reads comes back marker-free from a control that never held the
/// marker, exactly as it does from one the provider is withholding, so a plant
/// that silently failed would produce the same four green verdicts and the
/// report would be answering a question no fixture had asked. An unverified
/// plant is therefore a hard failure: the fixture refuses to become ready
/// rather than let a leak verdict rest on an absent secret.
fn plant_secret(password: HWND, secret: &str) -> Result<(), String> {
    let planted = wide(secret);
    if unsafe { SetWindowTextW(password, planted.as_ptr()) } == 0 {
        return Err("SetWindowTextW failed on the password control".into());
    }
    let mut buffer = vec![0u16; planted.len() + 16];
    let copied = unsafe { GetWindowTextW(password, buffer.as_mut_ptr(), buffer.len() as i32) };
    if copied <= 0 {
        return Err("the password control read back no text after the plant".into());
    }
    let readback = String::from_utf16_lossy(&buffer[..copied as usize]);
    if readback != secret {
        return Err(format!(
            "the password control holds {} characters that are not the marker",
            readback.chars().count()
        ));
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        return 0;
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn register_class(class_name: &str) -> Result<(), String> {
    let name = wide(class_name);
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(window_proc),
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        lpszClassName: name.as_ptr(),
        ..Default::default()
    };
    if unsafe { RegisterClassExW(&class) } == 0 {
        return Err("RegisterClassExW failed".into());
    }
    Ok(())
}

fn child(parent: HWND, class: &str, text: &str, style: u32, top: i32) -> HWND {
    let class = wide(class);
    let text = wide(text);
    unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            8,
            top,
            200,
            24,
            parent,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }
}

/// Creates the fixture window on the calling thread and pumps until it is
/// destroyed. The caller must be a thread that does nothing else, because
/// `ElementFromHandle` sends `WM_GETOBJECT` and blocks until this pump
/// dispatches it.
pub fn host_window(class_name: &str, secret: &str, ready: Sender<Result<isize, String>>) {
    if let Err(error) = register_class(class_name) {
        let _ = ready.send(Err(error));
        return;
    }
    let name = wide(class_name);
    let title = wide("agent-desktop probe fixture");
    let window = unsafe {
        CreateWindowExW(
            0,
            name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            2000,
            2000,
            420,
            320,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if window.is_null() {
        let _ = ready.send(Err("CreateWindowExW failed".into()));
        return;
    }
    child(window, "BUTTON", "probe-button", CONTROL_BORDER, 8);
    child(window, "STATIC", "probe-static", 0, 40);
    child(window, "EDIT", "probe-edit", CONTROL_BORDER, 72);
    let password = child(window, "EDIT", "", CONTROL_BORDER | ES_PASSWORD, 104);
    if let Err(error) = plant_secret(password, secret) {
        let _ = ready.send(Err(error));
        return;
    }
    unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };
    let _ = ready.send(Ok(window as isize));
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe { TranslateMessage(&message) };
        unsafe { DispatchMessageW(&message) };
    }
}

/// Joins the calling thread to the multithreaded apartment, the
/// precondition `UIAutomation::new_direct()` asserts rather than
/// establishes. Sub-phase 2.1 owns this step in the product.
pub fn join_multithreaded_apartment() -> i32 {
    use windows_sys::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) }
}

pub fn geometry(handle: isize) -> WindowGeometry {
    let window = handle as *mut c_void;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let read = unsafe { GetWindowRect(window, &mut rect) };
    WindowGeometry {
        visible: unsafe { IsWindowVisible(window) } != 0,
        width: if read != 0 { rect.right - rect.left } else { 0 },
        height: if read != 0 { rect.bottom - rect.top } else { 0 },
    }
}
