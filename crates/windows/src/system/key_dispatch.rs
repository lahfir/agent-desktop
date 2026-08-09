use std::time::{Duration, Instant};

use agent_desktop_core::{
    ActionResult, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    KeyCombo, ProcessId, ProcessIdentity,
};

use crate::input::elevation::ensure_target_integrity_allows_input;
use crate::input::keyboard::synthesize_key;
use crate::system::permissions::ensure_budget;
use crate::system::process_identity;

const KEYBOARD_FOCUS_WAIT: Duration = Duration::from_millis(500);
const KEYBOARD_FOCUS_POLL: Duration = Duration::from_millis(5);

/// Delivers a key combo to a named process by composing the keyboard primitive
/// under a verify-only focus gate. Core's headed path already activated before
/// this runs; this method never calls window activation.
pub(crate) fn press_for_app_impl(
    process: ProcessIdentity,
    combo: &KeyCombo,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    tracing::debug!(
        pid = u32::from(process.pid),
        key = combo.key.as_str(),
        allow_focus_steal = policy.allow_focus_steal,
        "system: press key for app"
    );
    ensure_budget(deadline)?;
    require_process(&process)?;
    ensure_injection_integrity(process.pid)?;
    ensure_foreground_for_delivery(&process, policy)?;
    require_process(&process)?;
    require_keyboard_focus(process.pid, &process.instance, deadline)?;
    require_process(&process)?;
    #[cfg(all(test, target_os = "windows"))]
    between_verify_and_inject::run();
    if !process_owns_foreground(process.pid, &process.instance)? {
        return Err(focus_lost_before_delivery(process.pid));
    }
    require_process(&process)?;
    run_synthesize(combo, deadline)?;
    post_delivery_process_result(&process)
}

fn ensure_injection_integrity(pid: ProcessId) -> Result<(), AdapterError> {
    #[cfg(all(test, target_os = "windows"))]
    if force_higher_integrity::is_active() {
        return crate::input::elevation::evaluate_integrity_gate(Some(0x2000), Some(0x3000));
    }
    ensure_target_integrity_allows_input(pid)
}

fn ensure_foreground_for_delivery(
    process: &ProcessIdentity,
    policy: InteractionPolicy,
) -> Result<(), AdapterError> {
    if process_owns_foreground(process.pid, &process.instance)? {
        return Ok(());
    }
    Err(if policy.allow_focus_steal {
        focus_verify_failed(process.pid)
    } else {
        background_fail_closed(process.pid)
    })
}

fn require_process(process: &ProcessIdentity) -> Result<(), AdapterError> {
    if process_identity::matches_instance(process.pid, &process.instance)? {
        Ok(())
    } else {
        Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Target process instance is no longer running",
        )
        .with_details(serde_json::json!({
            "pid": process.pid,
            "process_instance": process.instance,
            "complete": false,
            "physical_delivery_started": false,
        })))
    }
}

fn post_delivery_process_result(process: &ProcessIdentity) -> Result<ActionResult, AdapterError> {
    require_process(process)
        .map_err(|error| error.with_disposition(DeliverySemantics::delivered_unverified()))?;
    Ok(ActionResult::delivered_unverified("press_key"))
}

fn require_keyboard_focus(
    pid: ProcessId,
    instance: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let local_deadline = bounded_instant(deadline, KEYBOARD_FOCUS_WAIT)?;
    loop {
        ensure_budget(deadline)?;
        if process_holds_keyboard_focus(pid)?
            && process_identity::matches_instance(pid, instance)?
        {
            return Ok(());
        }
        if Instant::now() >= local_deadline {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Application has no verified keyboard focus for key delivery",
            )
            .with_details(serde_json::json!({
                "pid": pid,
                "physical_delivery_started": false,
            }))
            .with_disposition(DeliverySemantics::not_delivered()));
        }
        let pause = deadline.remaining_slice(KEYBOARD_FOCUS_POLL)?;
        std::thread::sleep(pause.min(KEYBOARD_FOCUS_POLL));
    }
}

fn bounded_instant(deadline: Deadline, maximum: Duration) -> Result<Instant, AdapterError> {
    let remaining = deadline.remaining_slice(maximum)?;
    Instant::now()
        .checked_add(remaining)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn run_synthesize(combo: &KeyCombo, deadline: Deadline) -> Result<(), AdapterError> {
    #[cfg(all(test, target_os = "windows"))]
    synthesis_probe::note();
    synthesize_key(combo, deadline)
}

fn focus_verify_failed(pid: ProcessId) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Target application lost focus before physical input delivery",
    )
    .with_details(serde_json::json!({
        "pid": pid,
        "physical_delivery_started": false,
    }))
    .with_suggestion("Retry after ensuring the target application remains frontmost")
    .with_disposition(DeliverySemantics::not_delivered())
}

fn background_fail_closed(pid: ProcessId) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Target application is not foreground; Windows SendInput cannot deliver to a background process",
    )
    .with_details(serde_json::json!({
        "pid": pid,
        "physical_delivery_started": false,
    }))
    .with_suggestion(
        "Retry with --headed so the target can be activated first, or ensure the target is already foreground",
    )
    .with_disposition(DeliverySemantics::not_delivered())
}

fn focus_lost_before_delivery(pid: ProcessId) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Target application lost focus before physical input delivery",
    )
    .with_details(serde_json::json!({
        "pid": pid,
        "physical_delivery_started": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(target_os = "windows")]
fn process_owns_foreground(pid: ProcessId, instance: &str) -> Result<bool, AdapterError> {
    let Some(foreground_pid) = foreground_process_id() else {
        return Ok(false);
    };
    if foreground_pid != pid {
        return Ok(false);
    }
    process_identity::matches_instance(pid, instance)
}

#[cfg(not(target_os = "windows"))]
fn process_owns_foreground(_pid: ProcessId, _instance: &str) -> Result<bool, AdapterError> {
    Ok(false)
}

#[cfg(target_os = "windows")]
fn process_holds_keyboard_focus(pid: ProcessId) -> Result<bool, AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetGUIThreadInfo, GUITHREADINFO};

    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    let ok = unsafe { GetGUIThreadInfo(0, &mut info) };
    if ok == 0 {
        return Ok(false);
    }
    let focus = if !info.hwndFocus.is_null() {
        info.hwndFocus
    } else if !info.hwndActive.is_null() {
        info.hwndActive
    } else {
        return Ok(false);
    };
    Ok(window_process_id(focus) == Some(pid))
}

#[cfg(not(target_os = "windows"))]
fn process_holds_keyboard_focus(_pid: ProcessId) -> Result<bool, AdapterError> {
    Ok(false)
}

#[cfg(target_os = "windows")]
fn foreground_process_id() -> Option<ProcessId> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_null() {
        return None;
    }
    window_process_id(foreground)
}

#[cfg(target_os = "windows")]
fn window_process_id(handle: windows_sys::Win32::Foundation::HWND) -> Option<ProcessId> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let mut pid = 0_u32;
    unsafe { GetWindowThreadProcessId(handle, &mut pid) };
    (pid != 0).then(|| ProcessId::from(pid))
}

#[cfg(all(test, target_os = "windows"))]
mod force_higher_integrity {
    use std::cell::Cell;

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn is_active() -> bool {
        ACTIVE.with(Cell::get)
    }

    pub(super) fn with<R>(run: impl FnOnce() -> R) -> R {
        struct ResetOnDrop;
        impl Drop for ResetOnDrop {
            fn drop(&mut self) {
                ACTIVE.with(|flag| flag.set(false));
            }
        }
        ACTIVE.with(|flag| flag.set(true));
        let _reset = ResetOnDrop;
        run()
    }
}

#[cfg(all(test, target_os = "windows"))]
mod between_verify_and_inject {
    use std::cell::RefCell;

    thread_local! {
        static HOOK: RefCell<Option<Box<dyn FnMut()>>> = RefCell::new(None);
    }

    pub(super) fn run() {
        HOOK.with(|cell| {
            if let Some(hook) = cell.borrow_mut().as_mut() {
                hook();
            }
        });
    }

    pub(super) fn with<R>(hook: impl FnMut() + 'static, run: impl FnOnce() -> R) -> R {
        struct ResetOnDrop;
        impl Drop for ResetOnDrop {
            fn drop(&mut self) {
                HOOK.with(|cell| *cell.borrow_mut() = None);
            }
        }
        HOOK.with(|cell| *cell.borrow_mut() = Some(Box::new(hook)));
        let _reset = ResetOnDrop;
        run()
    }
}

#[cfg(all(test, target_os = "windows"))]
mod synthesis_probe {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<u32> = const { Cell::new(0) };
    }

    pub(super) fn note() {
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(super) fn count() -> u32 {
        COUNT.with(Cell::get)
    }

    pub(super) fn reset() {
        COUNT.with(|cell| cell.set(0));
    }
}

#[cfg(test)]
#[path = "key_dispatch_tests.rs"]
mod tests;
