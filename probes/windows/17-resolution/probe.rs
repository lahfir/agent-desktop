//! Sub-phase 2.5 resolution probe (A17).
//!
//! Answers the resolution questions the 2.5 plan refuses to infer, on the
//! probe's own Win32 fixture, the WPF scratch window and the Chromium target
//! when present: FindAll versus the walk (U1-1), the live 0/1/N candidate
//! counts (U1-3), the single-element strict-resolve timing envelope (U1-4),
//! the shared single-element read cost (U1-5), the secure-field live-read
//! leak check (U1-6) and the ambiguity census (U1-7). The A7-3 fixture
//! reproduction (U1-2) and the Electron path/geometry survival leg (U1-8)
//! live in the orchestrator, which drives the WinForms fixture and Obsidian.
//!
//! Writes one JSON document to stdout. Nothing here is product code.

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

#[cfg(target_os = "windows")]
use uiautomation::UIAutomation;
#[cfg(target_os = "windows")]
use uiautomation::types::Handle;

#[cfg(target_os = "windows")]
#[path = "probe_window.rs"]
mod win;

#[cfg(target_os = "windows")]
#[path = "probe_measure.rs"]
mod measure;

#[cfg(target_os = "windows")]
#[path = "probe_findall.rs"]
mod findall;

#[cfg(target_os = "windows")]
#[path = "probe_swap.rs"]
mod swap;

#[cfg(target_os = "windows")]
#[path = "probe_survival.rs"]
mod survival;

const HOST_FLAG: &str = "--host";
const ATTACH_FLAG: &str = "--attach";
const OUT_FLAG: &str = "--out";
const HOST_HANDLE_PREFIX: &str = "HWND=";
const HOST_READY_TIMEOUT: Duration = Duration::from_secs(20);
const REPEATS: usize = 7;

fn flag_value(flag: &str, args: &mut env::Args) -> Option<String> {
    while let Some(argument) = args.next() {
        if argument == flag {
            return args.next();
        }
    }
    None
}

fn failure_shape(error: &uiautomation::Error) -> Value {
    json!({
        "code": error.code(),
        "result_hex": error.result().map(|hresult| format!("0x{:08X}", hresult.0 as u32)),
    })
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

fn min_of_ms(operation: impl FnMut() -> Result<(), ()>) -> (f64, f64, f64) {
    let mut samples = Vec::with_capacity(REPEATS);
    let mut closure = operation;
    let _ = closure();
    for _ in 0..REPEATS {
        let started = Instant::now();
        let _ = closure();
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    (samples[0], samples[samples.len() / 2], samples[samples.len() - 1])
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

    let arguments: Vec<String> = env::args().collect();
    let fd = |name: &str| -> Option<String> {
        arguments
            .iter()
            .position(|argument| argument == name)
            .and_then(|index| arguments.get(index + 1).cloned())
    };
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
                "child_process": { "hosted": true, "root_resolved": false, "root_failure": failure_shape(&error) },
            });
        }
    };

    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => {
            if let Some(child) = host.as_mut() {
                let _ = child.kill();
            }
            return json!({ "walker": { "failed": failure_shape(&error) } });
        }
    };

    let elements = measure::collect_descendants(&walker, &root, measure::WALK_DEPTH_LIMIT);
    let evidence: Vec<measure::Evidence> = elements.iter().map(measure::read_evidence).collect();

    let target = attached.as_deref().map(|_| "attached").unwrap_or("own-fixture");
    let census = measure::measure_census(&elements);

    // The live 0/1/N candidate counts over the fixture's own identifiers.
    let mut id_groups: std::collections::HashMap<(Option<String>, i32), usize> =
        std::collections::HashMap::new();
    for item in &evidence {
        *id_groups.entry((item.native_id.clone(), item.control_type)).or_insert(0) += 1;
    }
    let unique_key = id_groups
        .iter()
        .find(|((id, _), count)| id.is_some() && **count == 1)
        .map(|((id, role), _)| (id.clone().unwrap(), *role));
    let duplicate_key = id_groups
        .iter()
        .find(|((id, _), count)| id.is_some() && **count == 2)
        .map(|((id, role), _)| (id.clone().unwrap(), *role));

    let count_for = |id: &str, role: i32, name: Option<&str>| {
        evidence
            .iter()
            .filter(|item| {
                item.native_id.as_deref() == Some(id)
                    && item.control_type == role
                    && (name.is_none() || item.name.as_deref() == name)
            })
            .count()
    };
    let zero_one_n = json!({
        "unique_id_candidates": unique_key.as_ref().map(|(id, role)| count_for(id, *role, None)),
        "unique_key_present": unique_key.is_some(),
        "duplicate_id_candidates": duplicate_key.as_ref().map(|(id, role)| count_for(id, *role, None)),
        "duplicate_key_present": duplicate_key.is_some(),
        "absent_id_candidates": count_for("zz-probe-absent-id", 0, None),
    });

    // Single-element strict-resolve timing envelope (U1-4): one full
    // resolve-scoped walk plus the exact-match filter, per stored ref.
    let resolve_time = min_of_ms(|| {
        let mut read = 0usize;
        let _walk = measure::collect_descendants(&walker, &root, measure::WALK_DEPTH_LIMIT);
        for element in &_walk {
            let _ = measure::read_evidence(element);
            read += 1;
        }
        let _ = read;
        Ok(())
    });

    // Shared single-element read cost (U1-5): the evidence batch off one
    // element, min-of-seven.
    let read_time = min_of_ms(|| {
        if let Some(first) = elements.first() {
            let _ = measure::read_evidence(first);
        }
        Ok(())
    });

    // Secure-field live-read leak check (U1-6): find the password control by
    // its IsPassword flag, read Name and ValueValue, and test for the marker.
    let marker = "obs-pwd-marker-15ch";
    let secure = {
        let password = elements.iter().find(|element| {
            element
                .get_property_value(uiautomation::types::UIProperty::IsPassword)
                .ok()
                .and_then(|variant| measure::boolean_of(&variant))
                .unwrap_or(false)
        });
        match password {
            Some(element) => {
                let name = element
                    .get_property_value(uiautomation::types::UIProperty::Name)
                    .ok()
                    .and_then(|variant| variant.get_string().ok());
                let value = element
                    .get_property_value(uiautomation::types::UIProperty::ValueValue)
                    .ok()
                    .and_then(|variant| variant.get_string().ok());
                json!({
                    "password_control_present": true,
                    "name_contains_marker": name.map(|text| text.contains(marker)).unwrap_or(false),
                    "value_contains_marker": value.as_ref().map(|text| text.contains(marker)).unwrap_or(false),
                    "value_length": value.map(|text| text.chars().count()),
                })
            }
            None => json!({ "password_control_present": false }),
        }
    };

    let findall_pass = findall::measure_findall(&automation, &root, target);

    let document = json!({
        "child_process": {
            "hosted": attached.is_none(),
            "attached": attached.is_some(),
            "handle_int": handle,
            "root_resolved": true,
            "descendants": elements.len(),
        },
        "zero_one_n": zero_one_n,
        "secure_live_read": secure,
        "timing_ms": {
            "single_strict_resolve": {
                "min": resolve_time.0,
                "median": resolve_time.1,
                "max": resolve_time.2,
            },
            "single_element_read": {
                "min": read_time.0,
                "median": read_time.1,
                "max": read_time.2,
            },
        },
        "census": census,
        "findall_vs_walk": findall_pass,
    });

    if let Some(mut child) = host {
        let _ = child.kill();
        let _ = child.wait();
    }
    document
}

#[cfg(not(target_os = "windows"))]
fn measure() -> Value {
    json!({ "skipped": "this probe measures the Windows UI Automation runtime" })
}

const SWAP_ARM_FLAG: &str = "--swap-arm";
const SURVIVAL_ARM_FLAG: &str = "--survival-arm";

#[cfg(target_os = "windows")]
fn measure_swap(_prefix: &str) -> Value {
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
    let arguments: Vec<String> = env::args().collect();
    let handle = arguments
        .iter()
        .position(|argument| argument == SWAP_ARM_FLAG)
        .and_then(|index| arguments.get(index + 1))
        .and_then(|value| value.parse::<isize>().ok())
        .unwrap_or(0);
    let root = match automation.element_from_handle(Handle::from(handle)) {
        Ok(root) => root,
        Err(error) => {
            return json!({ "root_resolved": false, "root_failure": failure_shape(&error) });
        }
    };
    swap::measure_list_swap(&automation, &root, handle)
}


#[cfg(target_os = "windows")]
fn measure_survival(handle: isize) -> Value {
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
    let root = match automation.element_from_handle(Handle::from(handle)) {
        Ok(root) => root,
        Err(error) => {
            return json!({ "root_resolved": false, "root_failure": failure_shape(&error) });
        }
    };
    survival::measure_survival(&automation, &root)
}#[cfg(not(target_os = "windows"))]
fn measure_swap(_prefix: &str) -> Value {
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
            win::host_window("AgentDesktopResolutionHost", sender);
        }
        return;
    }
    let swap_prefix = env::args().find_map(|argument| {
        if argument == SWAP_ARM_FLAG {
            Some("winforms-fixture")
        } else {
            None
        }
    });
    let survival_handle = env::args()
        .position(|argument| argument == SURVIVAL_ARM_FLAG)
        .and_then(|index| env::args().nth(index + 1))
        .and_then(|value| value.parse::<isize>().ok());
    if let Some(handle) = survival_handle {
        let document = json!({
            "probe": "17-resolution",
            "uiautomation_version": option_env!("PROBE_UIAUTOMATION_VERSION").unwrap_or("unrecorded"),
            "measurements": measure_survival(handle),
        });
        let rendered = serde_json::to_string_pretty(&document).unwrap_or_default();
        println!("{rendered}");
        return;
    }
    if swap_prefix.is_some() {
        let prefix = swap_prefix.unwrap();
        let document = json!({
            "probe": "17-resolution",
            "uiautomation_version": option_env!("PROBE_UIAUTOMATION_VERSION").unwrap_or("unrecorded"),
            "measurements": measure_swap(prefix),
        });
        let rendered = serde_json::to_string_pretty(&document).unwrap_or_default();
        println!("{rendered}");
        return;
    }
    let document = json!({
        "probe": "17-resolution",
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