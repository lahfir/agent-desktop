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

#[cfg(target_os = "windows")]
#[path = "probe_run.rs"]
mod run;

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
    run::measure()
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