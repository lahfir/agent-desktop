//! Win32 fixture window for the 2.5 resolution probe.
//!
//! Creates the controls the resolution questions need a live tree for: a
//! statics row, buttons, an edit, an index-keyed list box (the A7-3 shape),
//! a password edit, a duplicate-evidence pair and a zero-extent control.
//! The duplicate pair is two `STATIC` children sharing the same control id and
//! text, so their UI Automation identity is indistinguishable: the live
//! two-candidate case the exact resolver must refuse. The password edit
//! carries a marker the fixture writes into the control text, so an un-gated
//! live read of a secure field is measurable.

use std::sync::mpsc::Sender;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, IDC_ARROW, LoadCursorW, MSG,
    PostQuitMessage, RegisterClassExW, SW_SHOWNOACTIVATE, SendMessageW, ShowWindow,
    TranslateMessage, WM_DESTROY, WNDCLASSEXW, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

const ES_MULTILINE: u32 = 0x0004;
const ES_PASSWORD: u32 = 0x0020;
const BS_AUTOCHECKBOX: u32 = 0x0003;
const BS_PUSHBUTTON: u32 = 0;
const LBS_HASSTRINGS: u32 = 0x0040;
const LBS_NOTIFY: u32 = 0x0001;
const LB_ADDSTRING: u32 = 0x0180;
const CONTROL_BORDER: u32 = 0x0080_0000;
const ROW_HEIGHT: i32 = 30;
const LIST_HEIGHT: i32 = 60;
const PASSWORD_MARKER: &str = "obs-pwd-marker-15ch";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
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

#[allow(clippy::too_many_arguments)]
fn child(
    parent: HWND,
    id: isize,
    class: &str,
    text: &str,
    style: u32,
    top: i32,
    width: i32,
    height: i32,
) -> HWND {
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
            width,
            height,
            parent,
            id as *mut _,
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }
}

fn add_list_item(list: HWND, text: &str) {
    let item = wide(text);
    unsafe { SendMessageW(list, LB_ADDSTRING, 0, item.as_ptr() as LPARAM) };
}

/// Creates the fixture window on the calling thread and pumps until it is
/// destroyed. The caller must be a thread that does nothing else, because
/// `ElementFromHandle` sends `WM_GETOBJECT` and blocks until this pump
/// dispatches it.
pub fn host_window(class_name: &str, ready: Sender<Result<isize, String>>) {
    if let Err(error) = register_class(class_name) {
        let _ = ready.send(Err(error));
        return;
    }
    let name = wide(class_name);
    let title = wide("agent-desktop resolution fixture");
    let window = unsafe {
        CreateWindowExW(
            0,
            name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            2000,
            2000,
            460,
            420,
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
    create_controls(window);
    unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };
    let _ = ready.send(Ok(window as isize));
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe { TranslateMessage(&message) };
        unsafe { DispatchMessageW(&message) };
    }
}

/// Joins the calling thread to the multithreaded apartment, the precondition
/// `UIAutomation::new_direct()` asserts rather than establishes.
pub fn join_multithreaded_apartment() -> i32 {
    use windows_sys::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) }
}

fn create_controls(window: HWND) {
    let mut top = 8;
    let mut row = |class: &str, text: &str, style: u32, height: i32| {
        let handle = child(window, top as isize, class, text, style, top, 220, height);
        top += height;
        handle
    };
    row("STATIC", "obs-static", 0, ROW_HEIGHT);
    row("BUTTON", "obs-button", CONTROL_BORDER | BS_PUSHBUTTON, ROW_HEIGHT);
    row("BUTTON", "obs-check", CONTROL_BORDER | BS_AUTOCHECKBOX, ROW_HEIGHT);
    row("EDIT", "obs-edit", CONTROL_BORDER, ROW_HEIGHT);
    row("EDIT", "obs-multiline", CONTROL_BORDER | ES_MULTILINE, LIST_HEIGHT);
    let password_top = top;
    child(
        window,
        password_top as isize,
        "EDIT",
        PASSWORD_MARKER,
        CONTROL_BORDER | ES_PASSWORD,
        top,
        220,
        ROW_HEIGHT,
    );
    top += ROW_HEIGHT;
    let list_top = top;
    let list = child(
        window,
        list_top as isize,
        "LISTBOX",
        "",
        CONTROL_BORDER | LBS_HASSTRINGS | LBS_NOTIFY,
        top,
        220,
        LIST_HEIGHT,
    );
    for item in ["obs-item-one", "obs-item-two", "obs-item-three"] {
        add_list_item(list, item);
    }
    top += LIST_HEIGHT;

    child(
        window,
        9999,
        "STATIC",
        "obs-dup",
        CONTROL_BORDER,
        top,
        90,
        ROW_HEIGHT,
    );
    child(
        window,
        9999,
        "STATIC",
        "obs-dup",
        CONTROL_BORDER,
        top,
        90,
        ROW_HEIGHT,
    );
    child(
        window,
        9998,
        "BUTTON",
        "zero",
        CONTROL_BORDER | BS_PUSHBUTTON,
        top,
        0,
        0,
    );
}