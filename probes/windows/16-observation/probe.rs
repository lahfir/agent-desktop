//! Sub-phase 2.4 observation probe.
//!
//! Answers the UIA-side observation questions the 2.4 plan refuses to infer:
//! what `LocalizedControlType` carries per control type, what `AriaRole` and
//! `AriaProperties` carry on a Chromium target and on native fixtures, the
//! marginal walk cost of adding the three properties, the transparent-wrapper
//! density on a Chromium tree, the connection-to-settled latency, and whether a
//! lowered-integrity client can still read a root and its process token.
//!
//! Writes one JSON document to stdout. Nothing here is product code.

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

#[cfg(target_os = "windows")]
use uiautomation::UIAutomation;
#[cfg(target_os = "windows")]
use uiautomation::types::Handle;



const HOST_FLAG: &str = "--host";
const ATTACH_FLAG: &str = "--attach";
const READ_HWND_FLAG: &str = "--read-hwnd";
const OUT_FLAG: &str = "--out";
const HOST_HANDLE_PREFIX: &str = "HWND=";
const HOST_READY_TIMEOUT: Duration = Duration::from_secs(20);
const WALK_DEPTH_LIMIT: u32 = 24;

#[cfg(target_os = "windows")]
#[path = "probe_window.rs"]
mod win;

#[cfg(target_os = "windows")]
#[path = "probe_properties.rs"]
mod properties;

#[cfg(target_os = "windows")]
#[path = "probe_measure.rs"]
mod measure;

#[cfg(target_os = "windows")]
#[path = "probe_cost.rs"]
mod cost;

fn flag_value(flag: &str, args: &mut env::Args) -> Option<String> {
    while let Some(argument) = args.next() {
        if argument == flag {
            return args.next();
        }
    }
    None
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
fn measure() -> Value {
    use uiautomation::UIAutomation;

    let apartment = win::join_multithreaded_apartment();
    let automation = match UIAutomation::new_direct() {
        Ok(automation) => automation,
        Err(error) => {
            return json!({
                "co_initialize_hresult": format!("0x{apartment:08X}"),
                "client": { "failed": measure::failure_shape(&error) },
            });
        }
    };

    let arguments: Vec<String> = env::args().collect();
    let fd = |name: &str| -> Option<String> {
        arguments
            .iter()
            .position(|argument| argument == name)
            .and_then(|index| arguments.get(index + 1).cloned())
    };
    if let Some(hwnd) = fd(READ_HWND_FLAG) {
        return read_hwnd_mode(&automation, &hwnd);
    }

    let attached = fd(ATTACH_FLAG);
    let (mut host, handle) = match &attached {
        Some(handle) => (None, handle.parse::<isize>().unwrap_or(0)),
        None => match spawn_host() {
            Ok((child, handle)) => (Some(child), handle),
            Err(error) => return json!({ "child_process": { "hosted": false, "error": error } }),
        },
    };
    let root = match automation.element_from_handle(Handle::from(handle)) {
        Ok(root) => root,
        Err(error) => {
            if let Some(child) = host.as_mut() {
                let _ = child.kill();
            }
            return json!({
                "child_process": { "hosted": true, "root_resolved": false, "root_failure": measure::failure_shape(&error) },
            });
        }
    };

    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => {
            if let Some(child) = host.as_mut() {
                let _ = child.kill();
            }
            return json!({ "walker": { "failed": measure::failure_shape(&error) } });
        }
    };

    let settle = if attached.is_some() {
        measure::settle_scan(handle, 12, Duration::from_millis(500), 3)
    } else {
        json!({ "walks": 0, "skipped": "own fixture needs no settle scan" })
    };
    let mut raw_depth = 0usize;
    let mut logical_depth = 0usize;
    if attached.is_some() {
        if let Ok(w) = automation.get_raw_view_walker() {
            if let Ok(r) = automation.element_from_handle(Handle::from(handle)) {
                measure::measure_depth_to_content(
                    &w,
                    &r,
                    0,
                    WALK_DEPTH_LIMIT,
                    &mut raw_depth,
                    &mut logical_depth,
                );
            }
        }
    }
    let elements = measure::collect_descendants(&walker, &root, WALK_DEPTH_LIMIT);
    let maybe_class = automation
        .element_from_handle(Handle::from(handle))
        .ok()
        .map(|root| measure::read_shape(&root, uiautomation::types::UIProperty::ClassName));
    let document = json!({
        "child_process": {
            "hosted": attached.is_none(),
            "attached": attached.is_some(),
            "handle_int": handle,
            "root_resolved": true,
            "descendants": elements.len(),
            "root_class": maybe_class,
        },
        "settle": settle,
        "depth_to_content": {
            "raw_nodes": raw_depth,
            "logical_nodes": logical_depth,
            "logical_over_raw": if raw_depth > 0 {
                (logical_depth as f64 / raw_depth as f64 * 100.0).round() / 100.0
            } else { 0.0 },
        },
        "localized_control_type": measure::measure_localized_control_type(&elements),
        "aria_properties": measure::measure_aria(&elements),
        "wrapper_census": measure::measure_wrapper_census(&elements),
        "pattern_arms": measure::measure_pattern_arms(&elements),
        "extra_cost": cost::measure_extra_cost(&automation, &root),
    });

    if let Some(mut child) = host {
        let _ = child.kill();
        let _ = child.wait();
    }
    document
}

/// Holds the root read and the token read for the split-integrity question:
/// whether a lowered-integrity caller can still observe a window and the
/// process-generation token that identifies it.
#[cfg(target_os = "windows")]
fn read_hwnd_mode(automation: &UIAutomation, hwnd: &str) -> Value {
    use std::time::Instant;

    let handle: isize = hwnd.parse().unwrap_or(0);
    if handle == 0 {
        return json!({ "root": { "resolved": false, "reason": "no hwnd" } });
    }
    let started = Instant::now();
    let root = automation.element_from_handle(Handle::from(handle));
    let root_elapsed_ms = started.elapsed().as_millis() as u64;
    let token = measure::process_token_of_window(handle);
    json!({
        "root": {
            "resolved": root.is_ok(),
            "elapsed_ms": root_elapsed_ms,
            "root_class": root.as_ref().ok().map(|element| {
                measure::read_shape(element, uiautomation::types::UIProperty::ClassName)
            }),
        },
        "process_token": token,
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
            win::host_window("AgentDesktopObservationHost", sender);
        }
        return;
    }
    let document = json!({
        "probe": "16-observation",
        "stack": "uia3-com",
        "uiautomation_version": option_env!("PROBE_UIAUTOMATION_VERSION").unwrap_or("unrecorded"),
        "measurements": measure(),
    });
    let rendered = serde_json::to_string_pretty(&document).unwrap_or_default();
    if let Some(path) = flag_value(OUT_FLAG, &mut env::args()) {
        if let Err(error) = std::fs::write(&path, rendered) {
            eprintln!("failed to write capture: {error}");
        }
    } else {
        println!("{rendered}");
    }
}
