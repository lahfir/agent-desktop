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
#[path = "probe_window.rs"]
mod win;

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
