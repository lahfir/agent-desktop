#![allow(dead_code)]

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, InteractionPolicy, SnapshotSurface, WindowInfo,
};

use super::permissions::ensure_budget;
use super::shell_surface::{
    SurfaceDismiss, SurfaceKindRow, SurfaceRaise, kebab, refusal_error, row_for,
};
#[cfg(all(test, target_os = "windows"))]
pub(in crate::system) use super::shell_surface_raise::accelerator_probe;
use super::shell_surface_raise::raise_row;

/// Opens a shell surface: refuses before anything is raised when the caller's
/// policy does not permit the foreground to move, returns an already-present
/// surface without raising, and otherwise raises and then waits for the
/// surface to be observed - never for the raise itself to report success.
pub(crate) fn open_surface(
    kind: SnapshotSurface,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    let Some(row) = row_for(kind) else {
        return Err(super::shell_surface::unknown_surface_error(kind));
    };
    require_foreground_policy(policy)?;
    open_row(row, deadline)
}

/// The caller's own policy answers whether the desktop's foreground may move,
/// and a chrome-raising command takes the foreground by definition - so the
/// refusal happens here, before the raise, not after it.
fn require_foreground_policy(policy: InteractionPolicy) -> Result<(), AdapterError> {
    if policy.allow_focus_steal {
        return Ok(());
    }
    Err(AdapterError::policy_denied_for_policy(
        "open a shell surface",
        policy,
    ))
}

pub(super) fn open_row(
    row: &SurfaceKindRow,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    ensure_budget(deadline)?;
    if !row.exists_on_build {
        return Err(refusal_error(row));
    }
    if let Some(existing) = super::shell_surface::resolve_row(row, deadline)? {
        return Ok(existing);
    }
    raise_row(row, deadline)?;
    poll_until_observed(row, deadline)
}

/// The open is never reported from the fact that the accelerator was sent -
/// the shell can simply decline - so every open ends in an observed resolve
/// or a `TIMEOUT`, polled at a bounded interval that grows the way the launch
/// observer's does rather than sleeping a fixed guess.
fn poll_until_observed(
    row: &SurfaceKindRow,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    let mut interval = std::time::Duration::from_millis(50);
    loop {
        if deadline.remaining().is_zero() {
            return Err(timeout_error(
                row,
                "did not open within the deadline",
                "shell_surface_not_opened",
            ));
        }
        ensure_budget(deadline)?;
        if let Some(observed) = super::shell_surface::resolve_row(row, deadline)? {
            return Ok(observed);
        }
        let remaining = deadline.remaining();
        std::thread::sleep(interval.min(remaining));
        interval = (interval * 3 / 2).min(std::time::Duration::from_millis(250));
    }
}

/// Dismisses a shell surface and returns once it is observed no longer
/// presented: unresolvable for the immersive family, whose liveness reads the
/// cloak, and hidden for the overflow, whose window class survives dismissal
/// the way the immersive ones do. Safe to call on an already-closed surface,
/// which is what makes entry-and-exit cleanup in callers idempotent.
pub(crate) fn close_surface(kind: SnapshotSurface, deadline: Deadline) -> Result<(), AdapterError> {
    let Some(row) = row_for(kind) else {
        return Err(super::shell_surface::unknown_surface_error(kind));
    };
    close_row(row, deadline)
}

#[cfg(target_os = "windows")]
pub(super) fn close_row(row: &SurfaceKindRow, deadline: Deadline) -> Result<(), AdapterError> {
    ensure_budget(deadline)?;
    if !surface_presented(row, deadline)? {
        return Ok(());
    }
    match row.dismiss {
        SurfaceDismiss::None => Ok(()),
        SurfaceDismiss::Escape => {
            super::shell_surface_raise::send_chord(
                &[],
                super::shell_surface_kinds::VK_ESCAPE,
                deadline,
            )?;
            poll_until_gone(row, deadline)
        }
        SurfaceDismiss::Toggle => {
            if let SurfaceRaise::Accelerator { modifiers, key } = row.raise {
                super::shell_surface_raise::send_chord(modifiers, key, deadline)?;
            }
            poll_until_gone(row, deadline)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) fn close_row(_row: &SurfaceKindRow, deadline: Deadline) -> Result<(), AdapterError> {
    ensure_budget(deadline)?;
    Err(AdapterError::not_supported("close shell surface"))
}

/// Whether the surface is presented to the user right now. The overflow's
/// window class stays materialized while closed (A26-6), so its presented
/// state is the top-level window's visibility; the immersive family's is the
/// resolver's own root-membership-and-cloak predicate (A26-2).
#[cfg(target_os = "windows")]
fn surface_presented(row: &SurfaceKindRow, deadline: Deadline) -> Result<bool, AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;

    ensure_budget(deadline)?;
    match &row.family {
        super::shell_surface::SurfaceFamily::Win32Class(chain) => {
            let Some(top) = super::shell_surface::class_chain_top_handle(chain) else {
                return Ok(false);
            };
            Ok(unsafe { IsWindowVisible(top) } != 0)
        }
        super::shell_surface::SurfaceFamily::Immersive { .. } => {
            Ok(super::shell_surface::resolve_row(row, deadline)?.is_some())
        }
    }
}

#[cfg(target_os = "windows")]
fn poll_until_gone(row: &SurfaceKindRow, deadline: Deadline) -> Result<(), AdapterError> {
    let mut interval = std::time::Duration::from_millis(50);
    loop {
        if deadline.remaining().is_zero() {
            return Err(timeout_error(
                row,
                "did not close within the deadline",
                "shell_surface_not_closed",
            ));
        }
        ensure_budget(deadline)?;
        if !surface_presented(row, deadline)? {
            return Ok(());
        }
        let remaining = deadline.remaining();
        std::thread::sleep(interval.min(remaining));
        interval = (interval * 3 / 2).min(std::time::Duration::from_millis(250));
    }
}

fn timeout_error(row: &SurfaceKindRow, outcome: &str, kind: &'static str) -> AdapterError {
    AdapterError::timeout(format!("the '{}' shell surface {outcome}", kebab(row.kind)))
        .with_suggestion(
            "Retry: the shell can decline an accelerator or an invoke without reporting it; \
         a build that lacks the surface refuses instead of timing out",
        )
        .with_details(serde_json::json!({ "kind": kind, "retryable": true }))
        .with_disposition(DeliverySemantics::not_delivered())
}
