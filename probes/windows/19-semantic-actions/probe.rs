//! Semantic action write-surface probe (A19).
//!
//! Measures Invoke/Toggle/SetValue/ExpandCollapse/Select/RangeValue on the
//! product CUIAutomation8 client, plus failure taxonomy, secure writes, UIPI,
//! SetFocus foreground effect, Legacy click, combobox/nested scroll, and cost.

use std::env;
use std::io::Write;

use serde_json::{Value, json};

#[cfg(target_os = "windows")]
#[path = "probe_util.rs"]
mod util;

#[cfg(target_os = "windows")]
#[path = "probe_ops.rs"]
mod ops;

#[cfg(target_os = "windows")]
#[path = "probe_semantic.rs"]
mod semantic;

#[cfg(target_os = "windows")]
#[path = "probe_failure.rs"]
mod failure;

#[cfg(target_os = "windows")]
#[path = "probe_secure.rs"]
mod secure;

#[cfg(target_os = "windows")]
#[path = "probe_focus.rs"]
mod focus;

#[cfg(target_os = "windows")]
#[path = "probe_legacy.rs"]
mod legacy;

#[cfg(target_os = "windows")]
#[path = "probe_combo.rs"]
mod combo;

#[cfg(target_os = "windows")]
#[path = "probe_cost.rs"]
mod cost;

#[cfg(target_os = "windows")]
#[path = "probe_uipi.rs"]
mod uipi;

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
    value
        .and_then(|text| text.parse::<isize>().ok())
        .filter(|handle| *handle != 0)
}

#[cfg(target_os = "windows")]
fn run() -> Value {
    let args: Vec<String> = env::args().collect();
    let automation = match util::bootstrap_product_client() {
        Ok(client) => client,
        Err(error) => return json!({ "client": error }),
    };

    let wpf = parse_hwnd(flag_value("--wpf", &args));
    let winforms = parse_hwnd(flag_value("--winforms", &args));
    let winforms_legacy = parse_hwnd(flag_value("--winforms-legacy", &args));
    let decoy = parse_hwnd(flag_value("--decoy", &args));

    if has_flag("--semantic", &args) {
        return semantic::measure(&automation, wpf, winforms);
    }
    if has_flag("--failure", &args) {
        return failure::measure(&automation, wpf);
    }
    if has_flag("--kill", &args) {
        let hwnd = parse_hwnd(flag_value("--kill-hwnd", &args)).unwrap_or(0);
        let pid = flag_value("--kill-pid", &args)
            .and_then(|text| text.parse().ok())
            .unwrap_or(0);
        let aid = flag_value("--kill-aid", &args).unwrap_or("txtValue");
        return failure::measure_killed(&automation, hwnd, pid, aid);
    }
    if has_flag("--secure", &args) {
        let doc = secure::measure(&automation, wpf, winforms);
        if secure::marker_leaked_in(&doc) {
            return json!({
                "fatal": "secure marker leaked into probe JSON before redaction",
                "doc_without_secret": "suppressed",
            });
        }
        return doc;
    }
    if has_flag("--focus", &args) {
        let hwnd = wpf.or(winforms).unwrap_or(0);
        return focus::measure(&automation, hwnd, decoy);
    }
    if has_flag("--legacy", &args) {
        return legacy::measure(&automation, winforms_legacy.or(winforms));
    }
    if has_flag("--combo", &args) {
        return combo::measure(&automation, wpf);
    }
    if has_flag("--cost", &args) {
        return cost::measure(&automation, wpf);
    }
    if has_flag("--uipi", &args) {
        let hwnd = parse_hwnd(flag_value("--high", &args)).unwrap_or(0);
        let value_id = flag_value("--value-id", &args).unwrap_or("txtValue");
        let invoke_id = flag_value("--invoke-id", &args).unwrap_or("btnAction");
        return uipi::measure(&automation, hwnd, value_id, invoke_id);
    }

    json!({
        "error": "no measurement flag",
        "flags": [
            "--semantic", "--failure", "--kill", "--secure", "--focus",
            "--legacy", "--combo", "--cost", "--uipi"
        ],
    })
}

#[cfg(not(target_os = "windows"))]
fn run() -> Value {
    json!({ "skipped": "non-windows host" })
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let doc = run();
    let encoded = serde_json::to_string_pretty(&doc).unwrap_or_else(|error| {
        json!({ "serialize_failed": error.to_string() }).to_string()
    });
    if let Some(path) = flag_value("--out", &args) {
        if let Err(error) = std::fs::write(path, &encoded) {
            let _ = writeln!(std::io::stderr(), "out_write_failed={}", error);
            std::process::exit(1);
        }
    }
    let _ = writeln!(std::io::stdout(), "{encoded}");
}
