use agent_desktop_core::{AdapterError, Deadline, DisplayInfo, ErrorCode, Rect};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_graphics::display::CGDisplay;

    pub fn list_displays_impl(deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError> {
        ensure_budget(deadline)?;
        let displays = CGDisplay::active_displays()
            .map_err(|_| AdapterError::internal("Failed to enumerate active displays"))?;
        ensure_budget(deadline)?;
        let main_id = CGDisplay::main().id;
        displays
            .into_iter()
            .map(|display_id| {
                ensure_budget(deadline)?;
                display_info(&CGDisplay::new(display_id), main_id)
            })
            .collect()
    }

    pub fn display_at(index: usize, deadline: Deadline) -> Result<DisplayInfo, AdapterError> {
        let displays = list_displays_impl(deadline)?;
        displays
            .into_iter()
            .nth(index)
            .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "display index out of range"))
    }

    pub fn scale_for_bounds(bounds: Option<Rect>, deadline: Deadline) -> Result<f64, AdapterError> {
        Ok(display_for_bounds(bounds, deadline)?.scale)
    }

    pub fn display_for_bounds(
        bounds: Option<Rect>,
        deadline: Deadline,
    ) -> Result<DisplayInfo, AdapterError> {
        let displays = list_displays_impl(deadline)?;
        select_display(&displays, bounds).cloned()
    }

    #[cfg(test)]
    fn scale_for_bounds_in(
        displays: &[DisplayInfo],
        bounds: Option<Rect>,
    ) -> Result<f64, AdapterError> {
        Ok(select_display(displays, bounds)?.scale)
    }

    fn select_display(
        displays: &[DisplayInfo],
        bounds: Option<Rect>,
    ) -> Result<&DisplayInfo, AdapterError> {
        let selected = bounds.and_then(|bounds| {
            displays
                .iter()
                .max_by(|left, right| {
                    intersection_area(bounds, left.bounds)
                        .total_cmp(&intersection_area(bounds, right.bounds))
                })
                .filter(|display| intersection_area(bounds, display.bounds) > 0.0)
        });
        selected
            .or_else(|| displays.iter().find(|display| display.is_primary))
            .or_else(|| displays.first())
            .ok_or_else(|| display_inventory_error("CoreGraphics returned no active displays"))
    }

    fn intersection_area(left: Rect, right: Rect) -> f64 {
        let width = (left.x + left.width).min(right.x + right.width) - left.x.max(right.x);
        let height = (left.y + left.height).min(right.y + right.height) - left.y.max(right.y);
        width.max(0.0) * height.max(0.0)
    }

    fn display_info(display: &CGDisplay, main_id: u32) -> Result<DisplayInfo, AdapterError> {
        let bounds = display.bounds();
        let mode = display.display_mode().ok_or_else(|| {
            display_inventory_error("CoreGraphics returned no current display mode")
        })?;
        let scale = scale_from_mode(mode.width() as f64, mode.pixel_width() as f64)?;
        Ok(DisplayInfo {
            id: display.id.to_string(),
            bounds: Rect {
                x: bounds.origin.x,
                y: bounds.origin.y,
                width: bounds.size.width,
                height: bounds.size.height,
            },
            is_primary: display.id == main_id,
            scale,
        })
    }

    fn scale_from_mode(point_width: f64, pixel_width: f64) -> Result<f64, AdapterError> {
        if !point_width.is_finite()
            || !pixel_width.is_finite()
            || point_width <= 0.0
            || pixel_width <= 0.0
        {
            return Err(display_inventory_error(
                "CoreGraphics returned invalid display mode dimensions",
            ));
        }
        Ok(pixel_width / point_width)
    }

    fn ensure_budget(deadline: Deadline) -> Result<(), AdapterError> {
        if deadline.is_expired() {
            Err(deadline.timeout_error())
        } else {
            Ok(())
        }
    }

    fn display_inventory_error(message: &str) -> AdapterError {
        AdapterError::new(ErrorCode::AppUnresponsive, message)
            .with_suggestion("Retry after WindowServer finishes updating the display inventory")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn intersection_area_selects_the_display_containing_most_of_a_window() {
            let window = Rect {
                x: 90.0,
                y: 0.0,
                width: 40.0,
                height: 50.0,
            };
            let left = Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            };
            let right = Rect {
                x: 100.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            };

            assert_eq!(intersection_area(window, left), 500.0);
            assert_eq!(intersection_area(window, right), 1_500.0);
        }

        #[test]
        fn window_capture_scale_comes_from_the_display_with_largest_overlap() {
            let displays = vec![
                DisplayInfo {
                    id: "main".into(),
                    bounds: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 100.0,
                        height: 100.0,
                    },
                    is_primary: true,
                    scale: 2.0,
                },
                DisplayInfo {
                    id: "external".into(),
                    bounds: Rect {
                        x: 100.0,
                        y: 0.0,
                        width: 100.0,
                        height: 100.0,
                    },
                    is_primary: false,
                    scale: 1.0,
                },
            ];
            let window = Rect {
                x: 90.0,
                y: 0.0,
                width: 40.0,
                height: 50.0,
            };

            assert_eq!(
                scale_for_bounds_in(&displays, Some(window)).expect("window display"),
                1.0
            );
            assert_eq!(
                scale_for_bounds_in(&displays, None).expect("primary display"),
                2.0
            );
        }

        #[test]
        fn missing_display_inventory_is_not_silently_scaled() {
            let error = scale_for_bounds_in(&[], None).expect_err("missing displays");

            assert_eq!(error.code, ErrorCode::AppUnresponsive);
        }

        #[test]
        fn mode_scale_is_orientation_independent() {
            assert_eq!(scale_from_mode(1440.0, 2880.0).unwrap(), 2.0);
            assert_eq!(scale_from_mode(900.0, 1800.0).unwrap(), 2.0);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn list_displays_impl(_deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_displays"))
    }

    pub fn display_at(_index: usize, _deadline: Deadline) -> Result<DisplayInfo, AdapterError> {
        Err(AdapterError::not_supported("list_displays"))
    }

    pub fn scale_for_bounds(
        _bounds: Option<Rect>,
        _deadline: Deadline,
    ) -> Result<f64, AdapterError> {
        Ok(1.0)
    }

    pub fn display_for_bounds(
        _bounds: Option<Rect>,
        _deadline: Deadline,
    ) -> Result<DisplayInfo, AdapterError> {
        Err(AdapterError::not_supported("list_displays"))
    }
}

pub(crate) use imp::{display_at, display_for_bounds, list_displays_impl, scale_for_bounds};
