//! Actionability unknowns probe (A18).
//!
//! Measures ScrollIntoView, ElementFromPoint corroboration, hang defense,
//! Unknown triggers, cost, envelope staging, Chromium hits, and the DPI
//! by-construction branch through the product's bounded CUIAutomation8 client.

use std::env;
use std::io::Write;
use std::process::Command;

use serde_json::{Value, json};

#[cfg(target_os = "windows")]
#[path = "probe_util.rs"]
mod util;

#[cfg(target_os = "windows")]
#[path = "probe_hit.rs"]
mod hit;

#[cfg(target_os = "windows")]
#[path = "probe_window.rs"]
mod win;

#[cfg(target_os = "windows")]
#[path = "probe_scroll.rs"]
mod scroll;

#[cfg(target_os = "windows")]
#[path = "probe_corroborate.rs"]
mod corroborate;

#[cfg(target_os = "windows")]
#[path = "probe_hang.rs"]
mod hang;

#[cfg(target_os = "windows")]
#[path = "probe_misc.rs"]
mod misc;

fn flag_value<'a>(flag: &str, args: &'a [String]) -> Option<&'a str> {
    args.iter()
        .position(|argument| argument == flag)
        .and_then(|index| args.get(index + 1).map(String::as_str))
}

fn has_flag(flag: &str, args: &[String]) -> bool {
    args.iter().any(|argument| argument == flag)
}

#[cfg(target_os = "windows")]
fn parse_hwnd(value: Option<&str>) -> Option<isize> {
    value.and_then(|text| text.parse::<isize>().ok()).filter(|h| *h != 0)
}

#[cfg(target_os = "windows")]
fn host_occluder_process(args: &[String]) -> Value {
    let x = flag_value("--x", args)
        .and_then(|t| t.parse().ok())
        .unwrap_or(140);
    let y = flag_value("--y", args)
        .and_then(|t| t.parse().ok())
        .unwrap_or(140);
    let hosted = match win::spawn_plain_window(
        "a18-foreign-host",
        x,
        y,
        400,
        280,
        0,
        windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
    ) {
        Ok(hosted) => hosted,
        Err(error) => {
            eprintln!("HOST_ERROR={error}");
            return json!({ "host_failed": error });
        }
    };
    println!("HWND={}", hosted.handle);
    let _ = std::io::Write::flush(&mut std::io::stdout());
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

#[cfg(target_os = "windows")]
fn run() -> Value {
    let args: Vec<String> = env::args().collect();

    if has_flag("--host", &args) {
        return host_occluder_process(&args);
    }

    let automation = match util::bootstrap_product_client() {
        Ok(client) => client,
        Err(error) => return json!({ "client": error }),
    };

    if has_flag("--dpi", &args) {
        return misc::measure_dpi();
    }
    if has_flag("--hang", &args) {
        return hang::measure_hang(&automation);
    }
    if has_flag("--unknown", &args) {
        return misc::measure_unknown(&automation);
    }

    let wpf = parse_hwnd(flag_value("--wpf", &args));
    let winforms = parse_hwnd(flag_value("--winforms", &args));
    let own = parse_hwnd(flag_value("--own", &args));
    let chromium = parse_hwnd(flag_value("--chromium", &args));
    let medium = parse_hwnd(flag_value("--medium", &args));
    let high = parse_hwnd(flag_value("--high", &args));
    let kill_hwnd = parse_hwnd(flag_value("--kill-scroll", &args));

    if has_flag("--scroll", &args) {
        let Some(hwnd) = wpf else {
            return json!({ "skipped": "scroll arm requires --wpf <hwnd>" });
        };
        return scroll::measure_scroll(&automation, hwnd);
    }
    if let Some(hwnd) = kill_hwnd {
        let pid = flag_value("--kill-pid", &args)
            .and_then(|text| text.parse::<u32>().ok())
            .unwrap_or(0);
        return scroll::measure_killed_provider(&automation, hwnd, || {
            if pid != 0 {
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        });
    }
    if has_flag("--corroborate", &args) {
        let foreign = parse_hwnd(flag_value("--foreign", &args));
        return corroborate::measure_corroborate(&automation, wpf, winforms, foreign);
    }
    if has_flag("--elevated", &args) {
        let (Some(medium), Some(high)) = (medium, high) else {
            return json!({ "skipped": "elevated arm requires --medium and --high" });
        };
        return corroborate::measure_elevated_pair(&automation, medium, high);
    }
    if has_flag("--envelope", &args) {
        let Some(hwnd) = wpf else {
            return json!({ "skipped": "envelope arm requires --wpf <hwnd>" });
        };
        return misc::measure_envelope(&automation, hwnd, winforms);
    }
    if has_flag("--cost", &args) {
        return misc::measure_cost(&automation, own, wpf, chromium);
    }
    if has_flag("--chromium-arm", &args) {
        let Some(hwnd) = chromium else {
            return json!({ "skipped": "chromium arm requires --chromium <hwnd>" });
        };
        return misc::measure_chromium(&automation, hwnd);
    }

    json!({
        "error": "no measurement arm selected",
        "arms": [
            "--scroll --wpf",
            "--kill-scroll --kill-pid",
            "--corroborate --wpf --winforms",
            "--elevated --medium --high",
            "--hang",
            "--unknown",
            "--envelope --wpf --winforms",
            "--cost --own --wpf --chromium",
            "--chromium-arm --chromium",
            "--dpi"
        ],
    })
}

#[cfg(not(target_os = "windows"))]
fn run() -> Value {
    json!({ "skipped": "this probe measures the Windows UI Automation runtime" })
}

fn main() {
    let doc = run();
    let encoded = serde_json::to_string_pretty(&doc).unwrap_or_else(|_| {
        "{\"ok\":false,\"error\":\"serialize_failed\"}".to_string()
    });
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{encoded}");
}
