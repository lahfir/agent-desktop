//! Opens Notification Center from the menu bar clock on the macOS 27 layout.
//!
//! macOS 27 hosts the status items in MenuBarAgent's `AXExtrasMenuBar`, where
//! each item sits inside an `AXHostingView` group, and Control Center no
//! longer exposes a menu bar for the System Events script to click. The clock
//! is found by its locale-independent identifier `com.apple.menuextra.clock`.
//! When MenuBarAgent is absent, the caller keeps the System Events script that
//! earlier layouts use.

use agent_desktop_core::{AdapterError, Deadline, Point, Rect};

/// Identifier the clock status item carries in MenuBarAgent.
pub(super) const CLOCK_IDENTIFIER: &str = "com.apple.menuextra.clock";

/// Process that hosts the status items on the macOS 27 layout.
pub(super) const CLOCK_HOST: &str = "MenuBarAgent";

/// Depth below the extras menu bar that the search walks; the clock sits at depth 1.
pub(super) const MAX_SEARCH_DEPTH: usize = 3;

/// How long the center has to appear after the click before the caller falls back.
const OPEN_CONFIRM_MS: u64 = 1_000;

/// Clicks the clock and reports whether Notification Center opened.
///
/// `AXPress` on the clock reports success on macOS 27 without opening the
/// center, so the click goes through the pointer, which only the headed policy
/// that reaches this function permits. The click is skipped, and `false`
/// returned, when the clock has no usable bounds or another element covers its
/// center, such as a frontmost window over an auto-hidden menu bar.
#[cfg(target_os = "macos")]
pub(super) fn open_from_clock(deadline: Deadline) -> Result<bool, AdapterError> {
    use agent_desktop_core::{
        MouseButton, MouseEvent, MouseEventKind, adapter::hit_test::HitTestResult,
    };

    let Some(clock) = find_clock(deadline)? else {
        return Ok(false);
    };
    let Some(point) =
        super::read::bounds(&clock, deadline)?.and_then(|bounds| click_point(&bounds))
    else {
        return Ok(false);
    };
    let hit = crate::tree::hit_test::hit_test_ax_element(&clock, point.clone(), deadline)?;
    if !matches!(hit, HitTestResult::ReachesTarget) {
        return Ok(false);
    }
    crate::input::mouse::synthesize_mouse(
        MouseEvent {
            kind: MouseEventKind::Click { count: 1 },
            point,
            button: MouseButton::Left,
            modifiers: Vec::new(),
        },
        deadline,
    )?;
    nc_opened_within(OPEN_CONFIRM_MS, deadline)
}

/// Polls for an open Notification Center for at most `window_ms`.
#[cfg(target_os = "macos")]
fn nc_opened_within(window_ms: u64, deadline: Deadline) -> Result<bool, AdapterError> {
    let confirm = deadline.slice(window_ms)?;
    loop {
        if let Some(pid) = super::nc_session::nc_pid(confirm)?
            && super::nc_session::is_nc_open(pid, confirm)
        {
            return Ok(true);
        }
        if confirm.is_expired() {
            return Ok(false);
        }
        std::thread::sleep(
            confirm
                .remaining()
                .min(std::time::Duration::from_millis(50)),
        );
    }
}

#[cfg(target_os = "macos")]
fn find_clock(deadline: Deadline) -> Result<Option<crate::tree::AXElement>, AdapterError> {
    let Some(pid) = super::nc_session::pid_of(CLOCK_HOST, deadline)? else {
        return Ok(None);
    };
    let Some(bar) = extras_menu_bar(&crate::tree::element_for_pid(pid), deadline)? else {
        return Ok(None);
    };
    find_by_identifier(
        bar,
        CLOCK_IDENTIFIER,
        MAX_SEARCH_DEPTH,
        |element| {
            tolerated(
                super::read::string(element, "AXIdentifier", deadline),
                deadline,
            )
            .map(Option::flatten)
        },
        |element| {
            tolerated(super::read::children(element, deadline), deadline)
                .map(Option::unwrap_or_default)
        },
    )
}

#[cfg(target_os = "macos")]
fn extras_menu_bar(
    app: &crate::tree::AXElement,
    deadline: Deadline,
) -> Result<Option<crate::tree::AXElement>, AdapterError> {
    crate::tree::properties::attributes::set_messaging_timeout(app, deadline)?;
    let result = crate::tree::properties::attributes::copy_element_attr_result(
        app,
        "AXExtrasMenuBar",
        deadline,
    );
    deadline.ensure_remaining()?;
    match result {
        Ok(bar) => Ok(bar),
        Err(code) if code == accessibility_sys::kAXErrorAPIDisabled => {
            Err(AdapterError::permission_denied())
        }
        Err(_) => Ok(None),
    }
}

/// Treats a read that one status item refuses as "not this item", so one
/// unreadable item does not hide the clock behind it.
#[cfg(target_os = "macos")]
fn tolerated<T>(
    result: Result<T, AdapterError>,
    deadline: Deadline,
) -> Result<Option<T>, AdapterError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) => super::read::tolerate_ax_strategy_error(error, deadline).map(|()| None),
    }
}

/// Depth-first search for the first node whose identifier equals `wanted`,
/// reading children no deeper than `max_depth` below `root`.
pub(super) fn find_by_identifier<T>(
    root: T,
    wanted: &str,
    max_depth: usize,
    mut identifier: impl FnMut(&T) -> Result<Option<String>, AdapterError>,
    mut children: impl FnMut(&T) -> Result<Vec<T>, AdapterError>,
) -> Result<Option<T>, AdapterError> {
    let mut pending = vec![(root, 0_usize)];
    while let Some((node, depth)) = pending.pop() {
        if identifier(&node)?.as_deref() == Some(wanted) {
            return Ok(Some(node));
        }
        if depth < max_depth {
            let next = children(&node)?;
            pending.extend(next.into_iter().rev().map(|child| (child, depth + 1)));
        }
    }
    Ok(None)
}

/// The center of `bounds`, or `None` when the rectangle has no area to click.
pub(super) fn click_point(bounds: &Rect) -> Option<Point> {
    (bounds.width > 0.0 && bounds.height > 0.0).then(|| Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    })
}

#[cfg(test)]
#[path = "clock_extra_tests.rs"]
mod tests;
