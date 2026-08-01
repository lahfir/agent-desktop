//! Win32 fixture window for the 2.3 vocabulary probe.
//!
//! Creates one control per vocabulary question the probe must answer: a
//! labelled edit and a labelled secure edit for `LabeledBy`, a read-only edit
//! for `ValueIsReadOnly`, a push button and an inert static for
//! `LegacyIAccessible.DefaultAction`, a checkbox for `ToggleToggleState`, and a
//! multi-select and a single-select list for `SelectionCanSelectMultiple`.
//!
//! Split from `probe.rs` to keep both files inside the repository's 400-line
//! source cap.

use std::sync::mpsc::Sender;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, IDC_ARROW, LoadCursorW, MSG,
    PostQuitMessage, RegisterClassExW, SW_SHOWNOACTIVATE, SendMessageW, SetWindowTextW, ShowWindow,
    TranslateMessage, WM_DESTROY, WNDCLASSEXW, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

const ES_PASSWORD: u32 = 0x0020;
const ES_READONLY: u32 = 0x0800;
const BS_AUTOCHECKBOX: u32 = 0x0003;
const LBS_MULTIPLESEL: u32 = 0x0008;
const LBS_HASSTRINGS: u32 = 0x0040;
const LBS_NOTIFY: u32 = 0x0001;
const LB_ADDSTRING: u32 = 0x0180;
const CONTROL_BORDER: u32 = 0x0080_0000;
const ROW_HEIGHT: i32 = 30;
const LIST_HEIGHT: i32 = 60;

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

fn child(parent: HWND, class: &str, text: &str, style: u32, top: i32, height: i32) -> HWND {
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
            220,
            height,
            parent,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }
}

fn add_list_item(list: HWND, text: &str) {
    let item = wide(text);
    unsafe { SendMessageW(list, LB_ADDSTRING, 0, item.as_ptr() as LPARAM) };
}

/// Creates the controls the vocabulary questions need, in the z-order the
/// Win32 client-side proxy derives `LabeledBy` from.
///
/// The read-only edit deliberately follows the secure edit with no static
/// between them, so the probe can measure whether a non-secure control can end
/// up labelled by a secure one - the cross-element case the per-element secure
/// gate does not cover.
fn create_controls(window: HWND, secret: &str) {
    let mut top = 8;
    let mut row = |class: &str, text: &str, style: u32, height: i32| {
        let handle = child(window, class, text, style, top, height);
        top += height;
        handle
    };
    row("STATIC", "vocab-label-one", 0, ROW_HEIGHT);
    row("EDIT", "vocab-edit-value", CONTROL_BORDER, ROW_HEIGHT);
    row("STATIC", "vocab-label-two", 0, ROW_HEIGHT);
    let password = row("EDIT", "", CONTROL_BORDER | ES_PASSWORD, ROW_HEIGHT);
    row(
        "EDIT",
        "vocab-readonly-value",
        CONTROL_BORDER | ES_READONLY,
        ROW_HEIGHT,
    );
    row("BUTTON", "vocab-button", CONTROL_BORDER, ROW_HEIGHT);
    row("STATIC", "vocab-static", 0, ROW_HEIGHT);
    row("BUTTON", "vocab-check", BS_AUTOCHECKBOX, ROW_HEIGHT);
    let multi = row(
        "LISTBOX",
        "",
        CONTROL_BORDER | LBS_HASSTRINGS | LBS_NOTIFY | LBS_MULTIPLESEL,
        LIST_HEIGHT,
    );
    let single = row(
        "LISTBOX",
        "",
        CONTROL_BORDER | LBS_HASSTRINGS | LBS_NOTIFY,
        LIST_HEIGHT,
    );
    let secret = wide(secret);
    unsafe { SetWindowTextW(password, secret.as_ptr()) };
    for list in [multi, single] {
        add_list_item(list, "vocab-item-one");
        add_list_item(list, "vocab-item-two");
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
    let title = wide("agent-desktop vocabulary fixture");
    let window = unsafe {
        CreateWindowExW(
            0,
            name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            2000,
            2000,
            460,
            520,
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
    create_controls(window, secret);
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
