use agent_desktop_core::{
    action::{DragParams, MouseButton, MouseEvent, MouseEventKind},
    error::AdapterError,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use core_graphics::geometry::CGPoint;

    pub fn synthesize_mouse(event: MouseEvent) -> Result<(), AdapterError> {
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
        let from = CGPoint::new(params.from.x, params.from.y);
        let to = CGPoint::new(params.to.x, params.to.y);
        let duration_ms = params.duration_ms.unwrap_or(300);
        let steps = (duration_ms / 16).max(4) as usize;
        let step_delay = std::time::Duration::from_millis(duration_ms / steps as u64);

        post_event(CGEventType::LeftMouseDown, from, CGMouseButton::Left)?;
        std::thread::sleep(std::time::Duration::from_millis(50));

        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let x = params.from.x + (params.to.x - params.from.x) * t;
            let y = params.from.y + (params.to.y - params.from.y) * t;
            post_event(CGEventType::LeftMouseDragged, CGPoint::new(x, y), CGMouseButton::Left)?;
            std::thread::sleep(step_delay);
        }

        post_event(CGEventType::LeftMouseUp, to, CGMouseButton::Left)
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
        unsafe {
            CGEventSetIntegerValueField(
                event as *const CGEvent as *const std::ffi::c_void,
                1,
                count,
            );
        }
    }

    extern "C" {
        fn CGEventSetIntegerValueField(
            event: *const std::ffi::c_void,
            field: u32,
            value: i64,
        );
    }

    fn create_event(
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<CGEvent, AdapterError> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|()| AdapterError::internal("Failed to create CGEventSource"))?;
        CGEvent::new_mouse_event(source, event_type, point, button)
            .map_err(|()| AdapterError::internal("CGEvent::new_mouse_event failed"))
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
}

pub use imp::{synthesize_drag, synthesize_mouse};
