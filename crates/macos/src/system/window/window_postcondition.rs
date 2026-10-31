use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Rect};

pub(crate) const TOLERANCE: f64 = 2.0;

pub(crate) fn wait_for_geometry(
    element: &crate::tree::AXElement,
    expected: Rect,
    position: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        let bounds = crate::tree::element_bounds::read_bounds_with_deadline(
            element,
            deadline_instant(deadline)?,
        )?;
        Ok(bounds.is_some_and(|actual| geometry_matches(actual, expected, position)))
    })
}

pub(crate) fn wait_for_minimized(
    element: &crate::tree::AXElement,
    expected: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        Ok(
            crate::tree::surface_read::boolean(
                element,
                "AXMinimized",
                deadline_instant(deadline)?,
            )? == Some(expected),
        )
    })
}

fn wait_until(
    deadline: Deadline,
    mut complete: impl FnMut() -> Result<bool, AdapterError>,
) -> Result<(), AdapterError> {
    loop {
        let transient = match complete() {
            Ok(true) => return Ok(()),
            Ok(false) => None,
            Err(error) if is_transient_read(&error) => Some(error),
            Err(error) => return Err(after_delivery(error)),
        };
        if deadline.is_expired() {
            return Err(after_delivery(transient.unwrap_or_else(|| {
                AdapterError::timeout("Window operation did not reach its requested postcondition")
            })));
        }
        let pause = deadline
            .remaining_slice(std::time::Duration::from_millis(10))
            .map_err(after_delivery)?;
        std::thread::sleep(pause.min(std::time::Duration::from_millis(10)));
    }
}

/// A window animating into or out of the Dock answers `AXMinimized` reads with
/// `kAXErrorCannotComplete` until the animation ends, so the postcondition
/// treats that read as unsettled and keeps polling until its deadline.
fn is_transient_read(error: &AdapterError) -> bool {
    error.details.as_ref().is_some_and(|details| {
        details.get("kind").and_then(|kind| kind.as_str()) == Some("surface_read")
            && details.get("ax_error").and_then(|code| code.as_i64())
                == Some(i64::from(CANNOT_COMPLETE))
    })
}

#[cfg(target_os = "macos")]
const CANNOT_COMPLETE: i32 = accessibility_sys::kAXErrorCannotComplete;

#[cfg(not(target_os = "macos"))]
const CANNOT_COMPLETE: i32 = -25204;

fn geometry_matches(actual: Rect, expected: Rect, position: bool) -> bool {
    let values = if position {
        [(actual.x, expected.x), (actual.y, expected.y)]
    } else {
        [
            (actual.width, expected.width),
            (actual.height, expected.height),
        ]
    };
    values
        .into_iter()
        .all(|(actual, expected)| (actual - expected).abs() <= TOLERANCE)
}

fn deadline_instant(deadline: Deadline) -> Result<std::time::Instant, AdapterError> {
    std::time::Instant::now()
        .checked_add(deadline.remaining())
        .ok_or_else(|| AdapterError::internal("Window operation deadline is out of range"))
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
#[path = "window_postcondition_tests.rs"]
mod tests;
