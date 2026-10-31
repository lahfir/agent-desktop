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
        let read =
            crate::tree::surface_read::boolean(element, "AXMinimized", deadline_instant(deadline)?);
        Ok(retry_while_animating(read)? == Some(expected))
    })
}

/// AppKit answers `kAXErrorCannotComplete` for `AXMinimized` while the minimize or restore
/// animation runs. That read is retried until the deadline instead of failing a delivered action.
fn retry_while_animating(
    read: Result<Option<bool>, AdapterError>,
) -> Result<Option<bool>, AdapterError> {
    match read {
        Err(error) if is_cannot_complete_read(&error) => Ok(None),
        other => other,
    }
}

fn is_cannot_complete_read(error: &AdapterError) -> bool {
    let Some(details) = error.details.as_ref() else {
        return false;
    };
    details.get("kind").and_then(serde_json::Value::as_str) == Some("surface_read")
        && details.get("ax_error").and_then(serde_json::Value::as_i64)
            == Some(i64::from(accessibility_sys::kAXErrorCannotComplete))
}

fn wait_until(
    deadline: Deadline,
    mut complete: impl FnMut() -> Result<bool, AdapterError>,
) -> Result<(), AdapterError> {
    loop {
        if complete().map_err(after_delivery)? {
            return Ok(());
        }
        if deadline.is_expired() {
            return Err(after_delivery(AdapterError::timeout(
                "Window operation did not reach its requested postcondition",
            )));
        }
        let pause = deadline
            .remaining_slice(std::time::Duration::from_millis(10))
            .map_err(after_delivery)?;
        std::thread::sleep(pause.min(std::time::Duration::from_millis(10)));
    }
}

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
mod tests {
    use super::*;

    #[test]
    fn geometry_verification_tolerates_ax_rounding_only() {
        let expected = Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
        };
        let rounded = Rect {
            x: 11.0,
            y: 19.0,
            width: 99.0,
            height: 81.0,
        };
        let wrong = Rect {
            width: 90.0,
            ..rounded
        };

        assert!(geometry_matches(rounded, expected, true));
        assert!(geometry_matches(rounded, expected, false));
        assert!(!geometry_matches(wrong, expected, false));
    }

    fn surface_read_error(ax_error: i32) -> AdapterError {
        AdapterError::new(
            agent_desktop_core::ErrorCode::Timeout,
            "Accessibility surface read failed for AXMinimized",
        )
        .with_details(serde_json::json!({
            "attribute": "AXMinimized",
            "ax_error": ax_error,
            "kind": "surface_read",
        }))
    }

    #[test]
    fn minimized_read_retries_cannot_complete_during_animation() {
        let read = Err(surface_read_error(
            accessibility_sys::kAXErrorCannotComplete,
        ));
        assert_eq!(retry_while_animating(read).unwrap(), None);
    }

    #[test]
    fn minimized_read_keeps_other_errors_and_values() {
        let invalid = Err(surface_read_error(
            accessibility_sys::kAXErrorInvalidUIElement,
        ));
        assert!(retry_while_animating(invalid).is_err());
        assert!(retry_while_animating(Err(AdapterError::timeout("deadline"))).is_err());
        assert_eq!(retry_while_animating(Ok(Some(true))).unwrap(), Some(true));
    }
}
