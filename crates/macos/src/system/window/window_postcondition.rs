use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Rect};

pub(crate) const TOLERANCE: f64 = 2.0;

/// How long a mismatching window frame must stay unchanged before the window server's
/// placement counts as final. macOS clamps off-screen or menu-bar positions and
/// minimum/maximum sizes immediately, so a stable mismatch is a refusal, not a delay.
pub(crate) const SETTLE: std::time::Duration = std::time::Duration::from_millis(400);

pub(crate) fn wait_for_geometry(
    element: &crate::tree::AXElement,
    expected: Rect,
    position: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let mut watch = GeometryWatch::new(expected, position);
    let mut last_seen = None;
    let outcome = wait_until(deadline, || {
        let bounds = crate::tree::element_bounds::read_bounds_with_deadline(
            element,
            deadline_instant(deadline)?,
        )?;
        last_seen = bounds.or(last_seen);
        watch.observe(bounds, std::time::Instant::now())
    });
    match outcome {
        Err(error) if error.code == agent_desktop_core::ErrorCode::Timeout => match last_seen {
            Some(actual) if !geometry_matches(actual, expected, position) => Err(after_delivery(
                placement_refused(actual, expected, position),
            )),
            _ => Err(error),
        },
        other => other,
    }
}

struct GeometryWatch {
    expected: Rect,
    position: bool,
    stable: Option<(Rect, std::time::Instant)>,
}

impl GeometryWatch {
    fn new(expected: Rect, position: bool) -> Self {
        Self {
            expected,
            position,
            stable: None,
        }
    }

    fn observe(
        &mut self,
        bounds: Option<Rect>,
        now: std::time::Instant,
    ) -> Result<bool, AdapterError> {
        let Some(actual) = bounds else {
            self.stable = None;
            return Ok(false);
        };
        if geometry_matches(actual, self.expected, self.position) {
            return Ok(true);
        }
        match self.stable {
            Some((seen, since)) if geometry_matches(actual, seen, self.position) => {
                if now.saturating_duration_since(since) >= SETTLE {
                    return Err(placement_refused(actual, self.expected, self.position));
                }
            }
            _ => self.stable = Some((actual, now)),
        }
        Ok(false)
    }
}

fn placement_refused(actual: Rect, expected: Rect, position: bool) -> AdapterError {
    let (what, requested, reached) = if position {
        (
            "position",
            serde_json::json!({ "x": expected.x, "y": expected.y }),
            serde_json::json!({ "x": actual.x, "y": actual.y }),
        )
    } else {
        (
            "size",
            serde_json::json!({ "width": expected.width, "height": expected.height }),
            serde_json::json!({ "width": actual.width, "height": actual.height }),
        )
    };
    let message = if position {
        format!(
            "macOS placed the window at x={}, y={} instead of the requested x={}, y={}",
            actual.x, actual.y, expected.x, expected.y
        )
    } else {
        format!(
            "macOS sized the window to {}x{} instead of the requested {}x{}",
            actual.width, actual.height, expected.width, expected.height
        )
    };
    AdapterError::new(agent_desktop_core::ErrorCode::ActionFailed, message)
        .with_suggestion(format!(
            "The window server or the app clamps this {what}: keep the window within the display work area (list-displays) and the app's size limits, then retry with a reachable value"
        ))
        .with_details(serde_json::json!({
            "kind": "window_geometry_clamped",
            "requested": requested,
            "actual": reached,
            "bounds": { "x": actual.x, "y": actual.y, "width": actual.width, "height": actual.height },
            "tolerance": TOLERANCE,
        }))
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

    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn stable_clamped_position_fails_fast_with_actual_position() {
        let start = std::time::Instant::now();
        let mut watch = GeometryWatch::new(rect(2500.0, 100.0, 0.0, 0.0), true);
        let clamped = Some(rect(1880.0, 100.0, 656.0, 422.0));
        assert!(!watch.observe(clamped, start).unwrap());
        assert!(
            !watch
                .observe(clamped, start + std::time::Duration::from_millis(200))
                .unwrap()
        );
        let error = watch.observe(clamped, start + SETTLE).unwrap_err();
        assert_eq!(error.code, agent_desktop_core::ErrorCode::ActionFailed);
        assert!(error.message.contains("x=1880"), "{}", error.message);
        let details = error.details.unwrap();
        assert_eq!(details["kind"], "window_geometry_clamped");
        assert_eq!(details["actual"]["x"], 1880.0);
        assert_eq!(details["requested"]["x"], 2500.0);
    }

    #[test]
    fn moving_window_restarts_the_settle_clock_and_match_wins() {
        let start = std::time::Instant::now();
        let mut watch = GeometryWatch::new(rect(0.0, 0.0, 800.0, 600.0), false);
        assert!(
            !watch
                .observe(Some(rect(0.0, 0.0, 500.0, 400.0)), start)
                .unwrap()
        );
        let later = start + SETTLE;
        assert!(
            !watch
                .observe(Some(rect(0.0, 0.0, 650.0, 500.0)), later)
                .unwrap()
        );
        assert!(!watch.observe(None, later + SETTLE).unwrap());
        assert!(
            watch
                .observe(Some(rect(0.0, 0.0, 799.0, 601.0)), later + SETTLE * 2)
                .unwrap()
        );
    }
}
