//! Sub-phase 2.2 CI capability probe.
//!
//! Measures the four facts the 2.2 plan refuses to infer: whether a window the
//! probe process creates itself is visible, non-zero-rect and UI Automation
//! walkable from a second MTA thread; the exact `code()`/`result()` pair the
//! real `uiautomation` crate returns at sibling exhaustion and at a forced
//! enumeration failure; and whether a password control leaks its content
//! through `Value`, `Name` or `HelpText`.
//!
//! Writes one JSON document to stdout. Nothing here is product code.

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Sender, channel};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use uiautomation::types::{Handle, UIProperty};
use uiautomation::{Error as UiaError, UIAutomation, UIElement, UITreeWalker};

const HOST_FLAG: &str = "--host";
const SECRET_MARKER: &str = "zzprobesecretzz";
const HOST_HANDLE_PREFIX: &str = "HWND=";
const WALK_DEPTH_LIMIT: u32 = 12;
const HOST_READY_TIMEOUT: Duration = Duration::from_secs(20);

#[cfg(target_os = "windows")]
mod win {
    use std::ffi::c_void;
    use std::sync::mpsc::Sender;

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowRect, IDC_ARROW,
        IsWindowVisible, LoadCursorW, MSG, PostQuitMessage, RegisterClassExW, SW_SHOWNOACTIVATE,
        SetWindowTextW, ShowWindow, TranslateMessage, WM_DESTROY, WNDCLASSEXW, WS_CHILD,
        WS_OVERLAPPEDWINDOW, WS_VISIBLE,
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
        let secret = wide(secret);
        unsafe { SetWindowTextW(password, secret.as_ptr()) };
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
}

fn failure_shape(error: &UiaError) -> Value {
    json!({
        "code": error.code(),
        "result_is_none": error.result().is_none(),
        "result_hex": error.result().map(|hresult| format!("0x{:08X}", hresult.0 as u32)),
    })
}

/// Enumerates children the way sub-phase 2.2's walker will, so the terminating
/// error is observed rather than swallowed by `UITreeWalker::get_children`.
fn enumerate_children(
    walker: &UITreeWalker,
    parent: &UIElement,
) -> (Vec<UIElement>, Option<UiaError>) {
    let mut children = Vec::new();
    let mut current = match walker.get_first_child(parent) {
        Ok(first) => first,
        Err(error) => return (children, Some(error)),
    };
    loop {
        let next = walker.get_next_sibling(&current);
        children.push(current);
        match next {
            Ok(sibling) => current = sibling,
            Err(error) => return (children, Some(error)),
        }
    }
}

fn count_descendants(walker: &UITreeWalker, root: &UIElement, depth: u32) -> u32 {
    if depth >= WALK_DEPTH_LIMIT {
        return 0;
    }
    let (children, _) = enumerate_children(walker, root);
    children.iter().fold(children.len() as u32, |total, child| {
        total + count_descendants(walker, child, depth + 1)
    })
}

fn last_descendant(walker: &UITreeWalker, root: &UIElement) -> UIElement {
    let (children, _) = enumerate_children(walker, root);
    match children.into_iter().next_back() {
        Some(child) => last_descendant(walker, &child),
        None => root.clone(),
    }
}

fn read_property(element: &UIElement, property: UIProperty) -> Value {
    match element.get_property_value(property) {
        Ok(value) => {
            let rendered = value.get_string().unwrap_or_default();
            json!({
                "variant_type": format!("{:?}", value.get_type()),
                "is_null": value.is_null(),
                "length": rendered.chars().count(),
                "contains_marker": rendered.contains(SECRET_MARKER),
            })
        }
        Err(error) => json!({ "failed": failure_shape(&error) }),
    }
}

fn password_leak_report(walker: &UITreeWalker, root: &UIElement) -> Value {
    let (children, _) = enumerate_children(walker, root);
    let secure = children.iter().find(|child| {
        child
            .get_property_value(UIProperty::IsPassword)
            .ok()
            .and_then(|value| value.get_value().ok())
            .is_some_and(|value| matches!(value, uiautomation::variants::Value::BOOL(true)))
    });
    match secure {
        None => json!({ "secure_field_found": false }),
        Some(element) => json!({
            "secure_field_found": true,
            "value": read_property(element, UIProperty::ValueValue),
            "legacy_value": read_property(element, UIProperty::LegacyIAccessibleValue),
            "name": read_property(element, UIProperty::Name),
            "help_text": read_property(element, UIProperty::HelpText),
        }),
    }
}

#[cfg(target_os = "windows")]
fn spawn_host() -> Result<(std::process::Child, isize), String> {
    let executable = env::current_exe().map_err(|error| error.to_string())?;
    let mut host = Command::new(executable)
        .arg(HOST_FLAG)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout = host.stdout.take().ok_or("host stdout unavailable")?;
    let (sender, receiver) = channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(handle) = line.strip_prefix(HOST_HANDLE_PREFIX) {
                let _ = sender.send(handle.trim().parse::<isize>().unwrap_or(0));
                return;
            }
        }
        let _ = sender.send(0);
    });
    match receiver.recv_timeout(HOST_READY_TIMEOUT) {
        Ok(handle) if handle != 0 => Ok((host, handle)),
        _ => {
            let _ = host.kill();
            Err("the host process never reported a window handle".into())
        }
    }
}

#[cfg(target_os = "windows")]
fn in_process_window(sender: Sender<Result<isize, String>>) {
    thread::spawn(move || win::host_window("AgentDesktopProbeLocal", SECRET_MARKER, sender));
}

#[cfg(target_os = "windows")]
fn measure() -> Value {
    let apartment = win::join_multithreaded_apartment();
    let automation = match UIAutomation::new_direct() {
        Ok(automation) => automation,
        Err(error) => {
            return json!({
                "co_initialize_hresult": format!("0x{apartment:08X}"),
                "client": { "failed": failure_shape(&error) },
            });
        }
    };
    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => return json!({ "walker": { "failed": failure_shape(&error) } }),
    };

    let (sender, receiver) = channel();
    in_process_window(sender);
    let local_handle = receiver
        .recv_timeout(HOST_READY_TIMEOUT)
        .unwrap_or_else(|_| Err("the in-process window never became ready".into()));
    let local = match local_handle {
        Ok(handle) => {
            let geometry = win::geometry(handle);
            let root = automation.element_from_handle(Handle::from(handle));
            json!({
                "created": true,
                "visible": geometry.visible,
                "non_zero_rect": geometry.width > 0 && geometry.height > 0,
                "root_resolved": root.is_ok(),
                "descendants_found": root
                    .as_ref()
                    .map(|element| count_descendants(&walker, element, 0))
                    .unwrap_or(0),
                "root_failure": root.err().as_ref().map(failure_shape),
            })
        }
        Err(error) => json!({ "created": false, "error": error }),
    };

    let (mut host, host_handle) = match spawn_host() {
        Ok(host) => host,
        Err(error) => {
            return json!({ "self_created_window": local, "child_process": { "hosted": false, "error": error } });
        }
    };
    let root = match automation.element_from_handle(Handle::from(host_handle)) {
        Ok(root) => root,
        Err(error) => {
            let _ = host.kill();
            return json!({
                "self_created_window": local,
                "child_process": { "hosted": true, "root_resolved": false, "root_failure": failure_shape(&error) },
            });
        }
    };
    let (children, terminator) = enumerate_children(&walker, &root);
    let exhaustion = terminator.as_ref().map(failure_shape);
    let secure = password_leak_report(&walker, &root);
    let retained = last_descendant(&walker, &root);
    let hosted = json!({
        "hosted": true,
        "root_resolved": true,
        "direct_children_found": children.len(),
        "descendants_found": count_descendants(&walker, &root, 0),
    });

    let _ = host.kill();
    let _ = host.wait();
    thread::sleep(Duration::from_millis(750));

    let forced_first_child = walker.get_first_child(&retained).err();
    let forced_sibling = walker.get_next_sibling(&retained).err();
    let stale_root = automation
        .element_from_handle(Handle::from(host_handle))
        .err();

    json!({
        "self_created_window": local,
        "child_process": hosted,
        "exhaustion": exhaustion,
        "secure_field": secure,
        "forced_failure": {
            "get_first_child": forced_first_child.as_ref().map(failure_shape),
            "get_next_sibling": forced_sibling.as_ref().map(failure_shape),
            "element_from_handle": stale_root.as_ref().map(failure_shape),
        },
    })
}

#[cfg(not(target_os = "windows"))]
fn measure() -> Value {
    json!({ "skipped": "this probe measures the Windows UI Automation runtime" })
}

fn main() {
    if env::args().any(|argument| argument == HOST_FLAG) {
        #[cfg(target_os = "windows")]
        {
            let (sender, receiver) = channel();
            thread::spawn(move || {
                if let Ok(Ok(handle)) = receiver.recv_timeout(HOST_READY_TIMEOUT) {
                    println!("{HOST_HANDLE_PREFIX}{handle}");
                    let _ = std::io::stdout().flush();
                }
            });
            win::host_window("AgentDesktopProbeHost", SECRET_MARKER, sender);
        }
        return;
    }
    let document = json!({
        "probe": "14-ci-capability",
        "stack": "uia3-com",
        "uiautomation_version": option_env!("PROBE_UIAUTOMATION_VERSION").unwrap_or("unrecorded"),
        "measurements": measure(),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&document).unwrap_or_default()
    );
}
