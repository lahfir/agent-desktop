//! The one seam that calls `SendInput` for mouse events.
//!
//! `SendInput`'s return value is never treated as delivery evidence (A9-3:
//! it reports success in both the delivered and UIPI-blocked arms), so this
//! module does not surface it as a `Result` - callers post best-effort and
//! judge effect upstream where a re-read exists.
//!
//! The real native call lives behind `#[cfg(not(test))]`, so no test in this
//! crate can reach it: that is a compile-time guarantee, not a runtime
//! check that a careless test could bypass. Test builds record into a
//! thread-local fake sink instead.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MouseInputEvent {
    pub(crate) dx: i32,
    pub(crate) dy: i32,
    pub(crate) mouse_data: i32,
    pub(crate) flags: u32,
}

pub(crate) fn post_mouse_inputs(events: &[MouseInputEvent]) {
    imp::post_mouse_inputs(events);
}

#[cfg(target_os = "windows")]
mod imp {
    use super::MouseInputEvent;

    pub(super) fn post_mouse_inputs(events: &[MouseInputEvent]) {
        #[cfg(test)]
        {
            super::mouse_send_fake_sink::record(events);
        }
        #[cfg(not(test))]
        {
            native::send(events);
        }
    }

    #[cfg(not(test))]
    mod native {
        use super::MouseInputEvent;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT, SendInput,
        };

        pub(super) fn send(events: &[MouseInputEvent]) {
            if events.is_empty() {
                return;
            }
            let raw: Vec<INPUT> = events.iter().copied().map(to_raw).collect();
            unsafe {
                SendInput(
                    raw.len() as u32,
                    raw.as_ptr(),
                    core::mem::size_of::<INPUT>() as i32,
                );
            }
        }

        fn to_raw(event: MouseInputEvent) -> INPUT {
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: event.dx,
                        dy: event.dy,
                        mouseData: event.mouse_data as u32,
                        dwFlags: event.flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::MouseInputEvent;

    pub(super) fn post_mouse_inputs(_events: &[MouseInputEvent]) {}
}

#[cfg(all(test, target_os = "windows"))]
pub(crate) mod mouse_send_fake_sink {
    use super::MouseInputEvent;
    use std::cell::RefCell;

    thread_local! {
        static RECORDED: RefCell<Vec<MouseInputEvent>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn reset() {
        RECORDED.with(|cell| cell.borrow_mut().clear());
    }

    pub(crate) fn recorded() -> Vec<MouseInputEvent> {
        RECORDED.with(|cell| cell.borrow().clone())
    }

    pub(super) fn record(events: &[MouseInputEvent]) {
        RECORDED.with(|cell| cell.borrow_mut().extend_from_slice(events));
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "mouse_send_tests.rs"]
mod tests;
