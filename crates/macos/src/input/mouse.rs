use agent_desktop_core::{
    action::{DragParams, MouseButton, MouseEvent, MouseEventKind},
    error::AdapterError,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_graphics::event::{
        CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField, ScrollEventUnit,
    };
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    pub fn synthesize_mouse(event: MouseEvent) -> Result<(), AdapterError> {
        tracing::debug!(
            "mouse: {:?} {:?} at ({:.0}, {:.0})",
            event.kind,
            event.button,
            event.point.x,
            event.point.y
        );
        let point = CGPoint::new(event.point.x, event.point.y);
        let cg_button = to_cg_button(&event.button);
        match event.kind {
            MouseEventKind::Move => post_event(CGEventType::MouseMoved, point, cg_button),
            MouseEventKind::Down => post_event(down_type(&event.button), point, cg_button),
            MouseEventKind::Up => post_event(up_type(&event.button), point, cg_button),
            MouseEventKind::Click { count } => {
                synthesize_click(point, cg_button, &event.button, count)
            }
        }
    }

    pub fn synthesize_drag(params: DragParams) -> Result<(), AdapterError> {
        drag_sequence(params).map_err(|err| {
            if err.suggestion.is_some() {
                return err;
            }
            err.with_suggestion(
                "The drag was aborted: the button was released back at the origin (best-effort) and no drop was committed at the destination. The cursor ends at the origin. Re-check the source state before retrying.",
            )
        })
    }

    fn drag_sequence(params: DragParams) -> Result<(), AdapterError> {
        tracing::debug!(
            "mouse: drag ({:.0},{:.0}) -> ({:.0},{:.0}) duration={}ms",
            params.from.x,
            params.from.y,
            params.to.x,
            params.to.y,
            params.duration_ms.unwrap_or(300)
        );
        use std::thread::sleep;
        use std::time::Duration;

        const PICKUP_DELAY_MS: u64 = 200;
        const DEFAULT_DROP_DELAY_MS: u64 = 500;
        const DWELL_TICK_MS: u64 = 16;

        let from = CGPoint::new(params.from.x, params.from.y);
        let to = CGPoint::new(params.to.x, params.to.y);
        let duration_ms = params.duration_ms.unwrap_or(300);
        let steps = (duration_ms / DWELL_TICK_MS).max(4) as usize;
        let step_delay = Duration::from_millis(duration_ms / steps as u64);
        let source = event_source()?;

        post_event_with_source(
            &source,
            CGEventType::LeftMouseDown,
            from,
            CGMouseButton::Left,
        )?;
        let mut release = MouseUpGuard {
            source: &source,
            origin: from,
            armed: true,
        };
        sleep(Duration::from_millis(PICKUP_DELAY_MS));

        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let x = params.from.x + (params.to.x - params.from.x) * t;
            let y = params.from.y + (params.to.y - params.from.y) * t;
            post_event_with_source(
                &source,
                CGEventType::LeftMouseDragged,
                CGPoint::new(x, y),
                CGMouseButton::Left,
            )?;
            sleep(step_delay);
        }

        dwell_over_destination(
            &source,
            to,
            params.drop_delay_ms.unwrap_or(DEFAULT_DROP_DELAY_MS),
            DWELL_TICK_MS,
        )?;
        release.release_at(to)
    }

    /// Releases the left mouse button exactly once. Every fallible step between
    /// `LeftMouseDown` and the final `LeftMouseUp` would otherwise leave the
    /// button logically held down system-wide on error. The happy path calls
    /// `release_at(to)`, which disarms only after the up event actually posts.
    /// On any early return, `Drop` cancels the gesture by dragging back to the
    /// origin and releasing there — never at the unreached destination, where
    /// CGEvent's embedded coordinates would silently commit the aborted drag
    /// as a completed drop. The cancel is best-effort twice over: the
    /// corrective posts themselves can fail (typically the same systemic
    /// CGEventSource failure that aborted the drag, leaving the button held
    /// and the cursor wherever the last successful event put it), and a
    /// drop target under the origin still sees a self-drop, which most
    /// targets treat as a no-op.
    struct MouseUpGuard<'a> {
        source: &'a CGEventSource,
        origin: CGPoint,
        armed: bool,
    }

    impl MouseUpGuard<'_> {
        fn release_at(&mut self, point: CGPoint) -> Result<(), AdapterError> {
            post_event_with_source(
                self.source,
                CGEventType::LeftMouseUp,
                point,
                CGMouseButton::Left,
            )?;
            self.armed = false;
            Ok(())
        }
    }

    impl Drop for MouseUpGuard<'_> {
        fn drop(&mut self) {
            if self.armed {
                let _ = post_event_with_source(
                    self.source,
                    CGEventType::LeftMouseDragged,
                    self.origin,
                    CGMouseButton::Left,
                );
                let _ = post_event_with_source(
                    self.source,
                    CGEventType::LeftMouseUp,
                    self.origin,
                    CGMouseButton::Left,
                );
            }
        }
    }

    /// Holds the dragged item over the destination while the drop target
    /// activates. Posting `LeftMouseDragged` on each tick (instead of a dead
    /// sleep) keeps the destination engaged so the release registers as a
    /// drop rather than a bare drag — macOS targets can drop the highlight if
    /// no movement arrives. A zero delay still posts one settling event.
    fn dwell_over_destination(
        source: &CGEventSource,
        to: CGPoint,
        drop_delay_ms: u64,
        tick_ms: u64,
    ) -> Result<(), AdapterError> {
        use std::thread::sleep;
        use std::time::Duration;

        let ticks = drop_delay_ms.div_ceil(tick_ms).max(1);
        for _ in 0..ticks {
            post_event_with_source(
                source,
                CGEventType::LeftMouseDragged,
                to,
                CGMouseButton::Left,
            )?;
            sleep(Duration::from_millis(tick_ms));
        }
        Ok(())
    }

    fn synthesize_click(
        point: CGPoint,
        cg_button: CGMouseButton,
        button: &MouseButton,
        count: u32,
    ) -> Result<(), AdapterError> {
        let down_ty = down_type(button);
        let up_ty = up_type(button);
        for i in 1..=count {
            let down = create_event(down_ty, point, cg_button)?;
            let up = create_event(up_ty, point, cg_button)?;
            set_click_count(&down, i as i64);
            set_click_count(&up, i as i64);
            down.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(10));
            up.post(CGEventTapLocation::HID);
            if i < count {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }
        Ok(())
    }

    fn set_click_count(event: &CGEvent, count: i64) {
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, count);
    }

    fn create_event(
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<CGEvent, AdapterError> {
        let source = event_source()?;
        create_event_with_source(&source, event_type, point, button)
    }

    fn create_event_with_source(
        source: &CGEventSource,
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<CGEvent, AdapterError> {
        CGEvent::new_mouse_event(source.clone(), event_type, point, button)
            .map_err(|()| AdapterError::internal("CGEvent::new_mouse_event failed"))
    }

    fn event_source() -> Result<CGEventSource, AdapterError> {
        CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|()| AdapterError::internal("Failed to create CGEventSource"))
    }

    fn post_event(
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<(), AdapterError> {
        let ev = create_event(event_type, point, button)?;
        ev.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn post_event_with_source(
        source: &CGEventSource,
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<(), AdapterError> {
        let ev = create_event_with_source(source, event_type, point, button)?;
        ev.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn to_cg_button(button: &MouseButton) -> CGMouseButton {
        match button {
            MouseButton::Left => CGMouseButton::Left,
            MouseButton::Right => CGMouseButton::Right,
            MouseButton::Middle => CGMouseButton::Center,
        }
    }

    fn down_type(button: &MouseButton) -> CGEventType {
        match button {
            MouseButton::Left => CGEventType::LeftMouseDown,
            MouseButton::Right => CGEventType::RightMouseDown,
            MouseButton::Middle => CGEventType::OtherMouseDown,
        }
    }

    fn up_type(button: &MouseButton) -> CGEventType {
        match button {
            MouseButton::Left => CGEventType::LeftMouseUp,
            MouseButton::Right => CGEventType::RightMouseUp,
            MouseButton::Middle => CGEventType::OtherMouseUp,
        }
    }

    pub fn synthesize_scroll_at(x: f64, y: f64, dy: i32, dx: i32) -> Result<(), AdapterError> {
        tracing::debug!("mouse: scroll at ({x:.0},{y:.0}) dy={dy} dx={dx}");
        use core_graphics::geometry::CGPoint;

        unsafe extern "C" {
            fn CGEventCreateScrollWheelEvent(
                source: *const std::ffi::c_void,
                units: u32,
                wheel_count: u32,
                wheel1: i32,
                wheel2: i32,
            ) -> *mut std::ffi::c_void;
            fn CGEventSetLocation(event: *mut std::ffi::c_void, point: CGPoint);
            fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
        }

        let event = unsafe {
            CGEventCreateScrollWheelEvent(std::ptr::null(), ScrollEventUnit::LINE, 2, dy, dx)
        };
        if event.is_null() {
            return Err(AdapterError::internal("scroll event creation failed"));
        }
        unsafe {
            CGEventSetLocation(event, CGPoint::new(x, y));
            CGEventPost(0, event);
            core_foundation::base::CFRelease(event as _);
        }
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn synthesize_mouse(_event: MouseEvent) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("mouse_event"))
    }

    pub fn synthesize_drag(_params: DragParams) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("drag"))
    }

    pub fn synthesize_scroll_at(_x: f64, _y: f64, _dy: i32, _dx: i32) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("scroll"))
    }
}

pub use imp::{synthesize_drag, synthesize_mouse, synthesize_scroll_at};
