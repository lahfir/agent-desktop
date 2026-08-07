//! Probe-owned Win32 fixtures: overlap pair, stalled host, style overlays.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::Arc;
use std::thread::{JoinHandle, spawn};
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, GetWindowRect,
    IDC_ARROW, LoadCursorW, MSG, PostMessageW, PostQuitMessage, RegisterClassExW,
    SetLayeredWindowAttributes, ShowWindow, TranslateMessage, UnregisterClassW, WM_CLOSE,
    WM_DESTROY, WNDCLASSEXW, WS_CHILD, WS_DISABLED, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    WS_OVERLAPPEDWINDOW, WS_POPUP, WS_VISIBLE, SW_MINIMIZE, SW_RESTORE, SW_SHOWNOACTIVATE,
};

const LWA_ALPHA: u32 = 0x0000_0002;
const BS_PUSHBUTTON: u32 = 0;

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

fn unregister_class(class_name: &str) {
    let name = wide(class_name);
    unsafe {
        UnregisterClassW(name.as_ptr(), GetModuleHandleW(std::ptr::null()));
    }
}

fn unique_class(prefix: &str) -> String {
    format!(
        "ad-a18-{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    )
}

pub fn minimize_restore(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd as HWND, SW_MINIMIZE);
        std::thread::sleep(Duration::from_millis(200));
        ShowWindow(hwnd as HWND, SW_RESTORE);
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub fn minimize_only(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd as HWND, SW_MINIMIZE);
    }
}

pub fn restore_only(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd as HWND, SW_RESTORE);
    }
}

pub struct HostedWindow {
    pub handle: isize,
    class_name: String,
    host: Option<JoinHandle<()>>,
}

impl Drop for HostedWindow {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                PostMessageW(self.handle as HWND, WM_CLOSE, 0, 0);
            }
        }
        if let Some(host) = self.host.take() {
            let _ = host.join();
        }
        unregister_class(&self.class_name);
    }
}

fn child_button(parent: HWND, id: isize, text: &str, x: i32, y: i32, w: i32, h: i32) -> HWND {
    let class = wide("BUTTON");
    let text = wide(text);
    unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
            x,
            y,
            w,
            h,
            parent,
            id as *mut _,
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }
}

fn pump_until_destroy(window: HWND) {
    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    let _ = window;
}

fn host_plain(
    class_name: &str,
    title: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    ex_style: u32,
    style: u32,
    ready: Sender<Result<isize, String>>,
) {
    if let Err(error) = register_class(class_name) {
        let _ = ready.send(Err(error));
        return;
    }
    let name = wide(class_name);
    let title = wide(title);
    let window = unsafe {
        CreateWindowExW(
            ex_style,
            name.as_ptr(),
            title.as_ptr(),
            style | WS_VISIBLE,
            x,
            y,
            w,
            h,
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
    child_button(window, 101, "target-btn", 20, 20, 120, 40);
    unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };
    if ex_style & WS_EX_LAYERED != 0 {
        unsafe {
            SetLayeredWindowAttributes(window, 0, 180, LWA_ALPHA);
        }
    }
    let _ = ready.send(Ok(window as isize));
    pump_until_destroy(window);
}

pub fn spawn_plain_window(
    title: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    ex_style: u32,
    style: u32,
) -> Result<HostedWindow, String> {
    let class_name = unique_class("plain");
    let (sender, receiver) = channel();
    let host = spawn({
        let class_name = class_name.clone();
        let title = title.to_string();
        move || host_plain(&class_name, &title, x, y, w, h, ex_style, style, sender)
    });
    match receiver.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(handle)) => Ok(HostedWindow {
            handle,
            class_name,
            host: Some(host),
        }),
        Ok(Err(error)) => Err(error),
        Err(_) => Err("plain window never became ready".into()),
    }
}

pub fn spawn_overlap_pair() -> Result<(HostedWindow, HostedWindow), String> {
    let under = spawn_plain_window(
        "a18-under",
        120,
        120,
        360,
        240,
        0,
        WS_OVERLAPPEDWINDOW,
    )?;
    let over = spawn_plain_window(
        "a18-over",
        200,
        160,
        360,
        240,
        0,
        WS_OVERLAPPEDWINDOW,
    )?;
    minimize_restore(over.handle);
    Ok((under, over))
}

pub fn spawn_layered_occluder(x: i32, y: i32) -> Result<HostedWindow, String> {
    spawn_plain_window(
        "a18-layered",
        x,
        y,
        280,
        180,
        WS_EX_LAYERED,
        WS_POPUP | WS_VISIBLE,
    )
}

pub fn spawn_transparent_occluder(x: i32, y: i32) -> Result<HostedWindow, String> {
    spawn_plain_window(
        "a18-transparent",
        x,
        y,
        280,
        180,
        WS_EX_LAYERED | WS_EX_TRANSPARENT,
        WS_POPUP | WS_VISIBLE,
    )
}

pub fn spawn_disabled_occluder(x: i32, y: i32) -> Result<HostedWindow, String> {
    spawn_plain_window(
        "a18-disabled",
        x,
        y,
        280,
        180,
        0,
        WS_POPUP | WS_VISIBLE | WS_DISABLED,
    )
}

pub struct StalledHost {
    pub handle: isize,
    class_name: String,
    stop: Arc<AtomicBool>,
    host: Option<JoinHandle<()>>,
}

impl Drop for StalledHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(host) = self.host.take() {
            let _ = host.join();
        }
        unregister_class(&self.class_name);
    }
}

fn host_stalled(class_name: &str, ready: Sender<Result<isize, String>>, stop: Arc<AtomicBool>) {
    if let Err(error) = register_class(class_name) {
        let _ = ready.send(Err(error));
        return;
    }
    let name = wide(class_name);
    let title = wide("a18-stalled");
    let window = unsafe {
        CreateWindowExW(
            0,
            name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            80,
            80,
            320,
            200,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if window.is_null() {
        let _ = ready.send(Err("stalled CreateWindowExW failed".into()));
        return;
    }
    unsafe { ShowWindow(window, SW_SHOWNOACTIVATE) };
    let _ = ready.send(Ok(window as isize));
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(50));
    }
    unsafe {
        DestroyWindow(window);
    }
}

pub fn spawn_stalled() -> Result<StalledHost, String> {
    let class_name = unique_class("stalled");
    let stop = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = channel();
    let host = spawn({
        let class_name = class_name.clone();
        let stop = stop.clone();
        move || host_stalled(&class_name, sender, stop)
    });
    match receiver.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(handle)) => Ok(StalledHost {
            handle,
            class_name,
            stop,
            host: Some(host),
        }),
        Ok(Err(error)) => Err(error),
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            Err("stalled window never became ready".into())
        }
    }
}

pub fn window_rect_csv(hwnd: isize) -> Option<String> {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetWindowRect(hwnd as HWND, &mut rect) } == 0 {
        return None;
    }
    Some(format!(
        "{},{},{},{}",
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top
    ))
}
