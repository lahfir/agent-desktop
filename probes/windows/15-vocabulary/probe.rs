//! Sub-phase 2.3 vocabulary probe.
//!
//! Answers the nine questions the 2.3 plan refuses to infer, against a fixture
//! hosted in a **child process** so every read crosses a real process boundary:
//! whether `LabeledBy` resolves and whether its target can be read; what
//! `HelpText`, `FullDescription` and `ItemStatus` carry on ordinary controls;
//! whether `LegacyIAccessible.DefaultAction` distinguishes an affordance-bearing
//! control from inert text; what `ValueIsReadOnly` and
//! `SelectionCanSelectMultiple` report; what `IsControlElement` and
//! `IsContentElement` report; what a pattern-state property returns on an
//! element lacking the pattern; the RawView-versus-ControlView delta; the
//! marginal cost of the expanded property set; and whether a cached
//! element-returning property carries the request's properties.
//!
//! Writes one JSON document to stdout. Nothing here is product code.

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

const HOST_FLAG: &str = "--host";
const ATTACH_FLAG: &str = "--attach";
const HOST_HANDLE_PREFIX: &str = "HWND=";
const HOST_READY_TIMEOUT: Duration = Duration::from_secs(20);

/// The window handle the caller asked the probe to measure instead of its own
/// fixture, so the same questions can be answered against a provider stack the
/// probe cannot create for itself.
fn attach_handle() -> Option<isize> {
    let mut arguments = env::args();
    while let Some(argument) = arguments.next() {
        if argument == ATTACH_FLAG {
            return arguments.next().and_then(|value| value.parse().ok());
        }
    }
    None
}

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
    use uiautomation::types::Handle;

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
    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => return json!({ "walker": { "failed": measure::failure_shape(&error) } }),
    };
    let attached = attach_handle();
    let (mut host, handle) = match attached {
        Some(handle) => (None, handle),
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
                "child_process": {
                    "hosted": true,
                    "root_resolved": false,
                    "root_failure": measure::failure_shape(&error),
                },
            });
        }
    };

    let elements = measure::collect_descendants(&walker, &root);
    let measurements = json!({
        "child_process": {
            "hosted": true,
            "attached": attached.is_some(),
            "root_resolved": true,
            "descendants": elements.len(),
        },
        "labeled_by": measure::measure_labeled_by(&elements),
        "per_control_type": measure::measure_per_control_type(&elements),
        "pattern_state_without_pattern": measure::measure_pattern_state_without_pattern(&elements),
        "view_delta": measure::measure_view_delta(&automation, &root),
        "batch_cost": cost::measure_batch_cost(&automation, &root),
        "cached_labeled_by": cost::measure_cached_labeled_by(&automation, &root),
    });

    if let Some(mut child) = host {
        let _ = child.kill();
        let _ = child.wait();
    }
    measurements
}

#[cfg(not(target_os = "windows"))]
fn measure() -> Value {
    json!({ "skipped": "this probe measures the Windows UI Automation runtime" })
}

/// Reports whether the secure control's content reached the capture, either as
/// a literal or through a read this probe classified as carrying it.
///
/// The probe's own gate: a leak here is a finding about UI Automation, not a
/// formatting detail, and it must be visible in the capture rather than
/// discovered by the corpus redaction pass.
fn marker_leaked(measurements: &Value) -> bool {
    let rendered = measurements.to_string();
    rendered.contains(marker()) || rendered.contains("\"contains_marker\":true")
}

#[cfg(target_os = "windows")]
fn marker() -> &'static str {
    measure::SECRET_MARKER
}

#[cfg(not(target_os = "windows"))]
fn marker() -> &'static str {
    "zzvocabsecretzz"
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
            win::host_window("AgentDesktopVocabularyHost", marker(), sender);
        }
        return;
    }
    let measurements = measure();
    let document = json!({
        "probe": "15-vocabulary",
        "stack": "uia3-com",
        "uiautomation_version": option_env!("PROBE_UIAUTOMATION_VERSION").unwrap_or("unrecorded"),
        "secure_content_leaked": marker_leaked(&measurements),
        "measurements": measurements,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&document).unwrap_or_default()
    );
}
