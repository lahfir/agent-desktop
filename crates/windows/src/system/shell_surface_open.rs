#![allow(dead_code)]

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy, SnapshotSurface,
    WindowInfo,
};

use super::permissions::ensure_budget;
use super::shell_surface::{
    SurfaceDismiss, SurfaceFamily, SurfaceKindRow, SurfaceRaise, kebab, refusal_error, row_for,
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

/// A resolved identity is only an open when the surface is actually
/// presented: the overflow's window class survives dismissal (A26-6), so its
/// resolver answers while hidden, and returning that hidden identity would
/// report an open the user cannot see - the raise the kind carries would
/// never fire. The taskbar family is always visible and the immersive
/// family's liveness already encodes presented (root membership AND an
/// uncloaked read, A26-2), so their resolved answer passes the presented
/// gate unchanged.
pub(super) fn open_row(
    row: &SurfaceKindRow,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    ensure_budget(deadline)?;
    if !row.exists_on_build {
        return Err(refusal_error(row));
    }
    if let Some(existing) = super::shell_surface::resolve_row(row, deadline)?
        && surface_presented(row, deadline)?
    {
        return Ok(existing);
    }
    let pre_raise_children = match &row.family {
        SurfaceFamily::Immersive { .. } => {
            super::shell_surface_immersive::witness_immersive_children()?
        }
        SurfaceFamily::Win32Class(_) => Vec::new(),
    };
    raise_row(row, deadline)?;
    poll_until_observed(row, deadline, &pre_raise_children)
}

/// The open is never reported from the fact that the accelerator was sent or
/// the chevron invoked - the shell can simply decline - so every open ends in
/// an observed, presented resolve or a `TIMEOUT`, polled at a bounded
/// interval that grows the way the launch observer's does rather than
/// sleeping a fixed guess. The resolve alone is not enough: it matches the
/// overflow's window while the surface is still hidden, so the presented
/// read rides alongside it. A raise that presents a same-class, same-host
/// surface matching none of the kind's landmarks resolves as the named
/// foreign-shape refusal instead of burning the deadline - the raise
/// presented something, and it is not the measured shape.
fn poll_until_observed(
    row: &SurfaceKindRow,
    deadline: Deadline,
    pre_raise_children: &[isize],
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
        if let Some(observed) = super::shell_surface::resolve_row(row, deadline)?
            && surface_presented(row, deadline)?
        {
            return Ok(observed);
        }
        if let SurfaceFamily::Immersive { landmarks, .. } = &row.family {
            let client = crate::tree::automation::automation_client()?;
            if super::shell_surface_immersive::raise_presented_foreign_shape(
                &client,
                pre_raise_children,
                landmarks,
            )? {
                return Err(super::shell_surface_immersive::foreign_shape_error(
                    landmarks,
                ));
            }
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
        SurfaceDismiss::RaiseThenEscape => raise_then_escape(row, deadline),
        SurfaceDismiss::Toggle => {
            if let SurfaceRaise::Accelerator { modifiers, key } = row.raise {
                super::shell_surface_raise::send_chord(modifiers, key, deadline)?;
            }
            poll_until_gone(row, deadline)
        }
    }
}

/// How long a re-raise is given to have been a toggle before the dismissal
/// falls through to Escape, and how long the surface is then given to take
/// the foreground that Escape needs. Both are short because both are the
/// shell's own response to a raise it accepted - measured landing inside one
/// 100ms poll - and the caller's deadline still bounds them.
#[cfg(target_os = "windows")]
const RAISE_SETTLE: std::time::Duration = std::time::Duration::from_millis(750);
#[cfg(target_os = "windows")]
const FOREGROUND_SETTLE: std::time::Duration = std::time::Duration::from_millis(1_500);

/// Escape with the precondition Escape silently depends on. A synthesized
/// Escape is delivered to whichever window owns the foreground, so sending
/// one at a surface that does not own it neither dismisses the surface nor
/// leaves the desktop alone - it goes into whatever the operator is using.
/// Re-running the shell's own raise settles both states the surface can be
/// in: it toggles a properly presented surface closed outright, and it
/// activates one left visible without activation, so the Escape that follows
/// reaches the surface rather than a bystander. A surface that stays up
/// without ever taking the foreground is refused by name instead of burning
/// the deadline, because no Escape this path could send would arrive.
#[cfg(target_os = "windows")]
fn raise_then_escape(row: &SurfaceKindRow, deadline: Deadline) -> Result<(), AdapterError> {
    super::shell_surface_raise::raise_row(row, deadline)?;
    if settles_absent(row, deadline)? {
        return Ok(());
    }
    await_foreground(row, deadline)?;
    super::shell_surface_raise::send_chord(&[], super::shell_surface_kinds::VK_ESCAPE, deadline)?;
    poll_until_gone(row, deadline)
}

#[cfg(target_os = "windows")]
fn settles_absent(row: &SurfaceKindRow, deadline: Deadline) -> Result<bool, AdapterError> {
    let start = std::time::Instant::now();
    loop {
        ensure_budget(deadline)?;
        if !surface_presented(row, deadline)? {
            return Ok(true);
        }
        let left = RAISE_SETTLE
            .saturating_sub(start.elapsed())
            .min(deadline.remaining());
        if left.is_zero() {
            return Ok(false);
        }
        std::thread::sleep(left.min(std::time::Duration::from_millis(50)));
    }
}

#[cfg(target_os = "windows")]
fn await_foreground(row: &SurfaceKindRow, deadline: Deadline) -> Result<(), AdapterError> {
    let start = std::time::Instant::now();
    loop {
        ensure_budget(deadline)?;
        if surface_owns_foreground(row, deadline)? {
            return Ok(());
        }
        let left = FOREGROUND_SETTLE
            .saturating_sub(start.elapsed())
            .min(deadline.remaining());
        if left.is_zero() {
            return Err(foreground_declined_error(row));
        }
        std::thread::sleep(left.min(std::time::Duration::from_millis(50)));
    }
}

/// Whether a keystroke synthesized now would land in this surface. The
/// foreground window can be a descendant of the surface rather than the
/// surface itself - the Start overlay's foreground is the search input's own
/// window inside it (A26-9) - so ownership is read at the foreground window's
/// root rather than by equality with it.
#[cfg(target_os = "windows")]
fn surface_owns_foreground(row: &SurfaceKindRow, deadline: Deadline) -> Result<bool, AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetForegroundWindow};

    let Some(top) = surface_top_handle(row, deadline)? else {
        return Ok(false);
    };
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_null() {
        return Ok(false);
    }
    Ok(unsafe { GetAncestor(foreground, GA_ROOT) } == top)
}

#[cfg(target_os = "windows")]
fn surface_top_handle(
    row: &SurfaceKindRow,
    deadline: Deadline,
) -> Result<Option<super::window_enum::WindowHandle>, AdapterError> {
    match &row.family {
        SurfaceFamily::Win32Class(chain) => Ok(super::shell_surface::class_chain_top_handle(chain)),
        SurfaceFamily::Immersive { .. } => Ok(super::shell_surface::resolve_row(row, deadline)?
            .map(|info| super::window_ops::parse_handle(&info.id))
            .filter(|handle| !handle.is_null())),
    }
}

/// The answer for a surface that is up but unreachable by keystroke. It is
/// not a timeout: waiting longer cannot change it, because the surface never
/// took the foreground the dismissal needs and the deadline is not what is
/// short. Naming the state lets a caller act on it instead of retrying.
#[cfg(target_os = "windows")]
fn foreground_declined_error(row: &SurfaceKindRow) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "the '{}' shell surface stayed up without taking the foreground, \
             so a dismissal keystroke cannot reach it",
            kebab(row.kind)
        ),
    )
    .with_suggestion(
        "Activate the surface - click it, or raise it again from the shell - then retry the close",
    )
    .with_details(serde_json::json!({
        "kind": "shell_surface_declined_foreground",
        "retryable": true
    }))
    .with_disposition(DeliverySemantics::not_delivered())
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

#[cfg(all(test, target_os = "windows"))]
#[path = "shell_surface_command_live_tests.rs"]
mod command_live_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "shell_surface_close_tests.rs"]
mod close_tests;
