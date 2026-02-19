use agent_desktop_core::{action::KeyCombo, error::AdapterError};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_graphics::event::{
        CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode,
    };
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    pub fn synthesize_key(combo: &KeyCombo) -> Result<(), AdapterError> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| AdapterError::internal("Failed to create CGEventSource"))?;

        let key_code = key_name_to_code(&combo.key)?;
        let mut flags = CGEventFlags::empty();

        for modifier in &combo.modifiers {
            flags |= modifier_to_flags(modifier);
        }

        let key_down = CGEvent::new_keyboard_event(source.clone(), key_code, true)
            .map_err(|_| AdapterError::internal("Failed to create key down event"))?;
        key_down.set_flags(flags);

        let key_up = CGEvent::new_keyboard_event(source, key_code, false)
            .map_err(|_| AdapterError::internal("Failed to create key up event"))?;
        key_up.set_flags(flags);

        key_down.post(CGEventTapLocation::HID);
        key_up.post(CGEventTapLocation::HID);
        Ok(())
    }

    pub fn synthesize_text(text: &str) -> Result<(), AdapterError> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| AdapterError::internal("Failed to create CGEventSource"))?;

        for ch in text.chars() {
            let key_down = CGEvent::new_keyboard_event(source.clone(), 0, true)
                .map_err(|_| AdapterError::internal("Failed to create keyboard event"))?;
            key_down.set_string(&ch.to_string());
            key_down.post(CGEventTapLocation::HID);

            let key_up = CGEvent::new_keyboard_event(source.clone(), 0, false)
                .map_err(|_| AdapterError::internal("Failed to create keyboard event"))?;
            key_up.post(CGEventTapLocation::HID);
        }
        Ok(())
    }

    fn modifier_to_flags(m: &agent_desktop_core::action::Modifier) -> CGEventFlags {
        use agent_desktop_core::action::Modifier;
        match m {
            Modifier::Cmd => CGEventFlags::CGEventFlagCommand,
            Modifier::Ctrl => CGEventFlags::CGEventFlagControl,
            Modifier::Alt => CGEventFlags::CGEventFlagAlternate,
            Modifier::Shift => CGEventFlags::CGEventFlagShift,
        }
    }

    fn key_name_to_code(key: &str) -> Result<CGKeyCode, AdapterError> {
        let code = match key {
            "a" => 0, "b" => 11, "c" => 8, "d" => 2, "e" => 14, "f" => 3,
            "g" => 5, "h" => 4, "i" => 34, "j" => 38, "k" => 40, "l" => 37,
            "m" => 46, "n" => 45, "o" => 31, "p" => 35, "q" => 12, "r" => 15,
            "s" => 1, "t" => 17, "u" => 32, "v" => 9, "w" => 13, "x" => 7,
            "y" => 16, "z" => 6,
            "0" => 29, "1" => 18, "2" => 19, "3" => 20, "4" => 21,
            "5" => 23, "6" => 22, "7" => 26, "8" => 28, "9" => 25,
            "return" | "enter" => 36,
            "escape" | "esc" => 53,
            "tab" => 48,
            "space" => 49,
            "delete" | "backspace" => 51,
            "left" => 123, "right" => 124, "down" => 125, "up" => 126,
            "f1" => 122, "f2" => 120, "f3" => 99, "f4" => 118,
            "f5" => 96, "f6" => 97, "f7" => 98, "f8" => 100,
            "f9" => 101, "f10" => 109, "f11" => 103, "f12" => 111,
            other => return Err(AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                format!("Unknown key: '{other}'"),
            )),
        };
        Ok(code)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn synthesize_key(_combo: &KeyCombo) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("synthesize_key"))
    }

    pub fn synthesize_text(_text: &str) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("synthesize_text"))
    }
}

pub use imp::{synthesize_key, synthesize_text};
