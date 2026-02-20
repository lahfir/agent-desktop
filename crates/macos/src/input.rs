use agent_desktop_core::{action::KeyCombo, error::AdapterError};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use accessibility_sys::{
        kAXErrorSuccess, AXUIElementCreateSystemWide, AXUIElementPostKeyboardEvent,
    };

    pub fn synthesize_key(combo: &KeyCombo) -> Result<(), AdapterError> {
        let key_code = key_name_to_code(&combo.key)?;

        let sys_wide = unsafe { AXUIElementCreateSystemWide() };
        if sys_wide.is_null() {
            return Err(AdapterError::internal("Failed to create system-wide AX element"));
        }

        if !combo.modifiers.is_empty() {
            for m in &combo.modifiers {
                let mod_code = modifier_keycode(m);
                let err = unsafe { AXUIElementPostKeyboardEvent(sys_wide, 0, mod_code, true) };
                if err != kAXErrorSuccess {
                    release_modifiers(sys_wide, &combo.modifiers);
                    unsafe { core_foundation::base::CFRelease(sys_wide as _) };
                    return Err(AdapterError::internal(format!(
                        "AXUIElementPostKeyboardEvent modifier-down failed (err={err})"
                    )));
                }
            }
        }

        let err_down = unsafe { AXUIElementPostKeyboardEvent(sys_wide, 0, key_code, true) };
        let err_up = unsafe { AXUIElementPostKeyboardEvent(sys_wide, 0, key_code, false) };

        if !combo.modifiers.is_empty() {
            release_modifiers(sys_wide, &combo.modifiers);
        }

        unsafe { core_foundation::base::CFRelease(sys_wide as _) };

        if err_down != kAXErrorSuccess {
            return Err(AdapterError::internal(format!(
                "AXUIElementPostKeyboardEvent key-down failed (err={err_down})"
            )));
        }
        if err_up != kAXErrorSuccess {
            return Err(AdapterError::internal(format!(
                "AXUIElementPostKeyboardEvent key-up failed (err={err_up})"
            )));
        }
        Ok(())
    }

    pub fn synthesize_text(text: &str) -> Result<(), AdapterError> {
        let sys_wide = unsafe { AXUIElementCreateSystemWide() };
        if sys_wide.is_null() {
            return Err(AdapterError::internal("Failed to create system-wide AX element"));
        }

        for ch in text.chars() {
            if ch == '\n' {
                let return_code = 36u16;
                unsafe {
                    AXUIElementPostKeyboardEvent(sys_wide, 0, return_code, true);
                    AXUIElementPostKeyboardEvent(sys_wide, 0, return_code, false);
                };
            } else if let Some(code) = char_to_keycode(ch) {
                let needs_shift = ch.is_ascii_uppercase() || is_shifted_char(ch);
                if needs_shift {
                    unsafe { AXUIElementPostKeyboardEvent(sys_wide, 0, 56, true) };
                }
                unsafe {
                    AXUIElementPostKeyboardEvent(sys_wide, 0, code, true);
                    AXUIElementPostKeyboardEvent(sys_wide, 0, code, false);
                };
                if needs_shift {
                    unsafe { AXUIElementPostKeyboardEvent(sys_wide, 0, 56, false) };
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(4));
        }

        unsafe { core_foundation::base::CFRelease(sys_wide as _) };
        Ok(())
    }

    fn release_modifiers(
        el: accessibility_sys::AXUIElementRef,
        modifiers: &[agent_desktop_core::action::Modifier],
    ) {
        for m in modifiers.iter().rev() {
            let code = modifier_keycode(m);
            unsafe { AXUIElementPostKeyboardEvent(el, 0, code, false) };
        }
    }

    fn modifier_keycode(m: &agent_desktop_core::action::Modifier) -> u16 {
        use agent_desktop_core::action::Modifier;
        match m {
            Modifier::Cmd => 55,
            Modifier::Shift => 56,
            Modifier::Alt => 58,
            Modifier::Ctrl => 59,
        }
    }

    fn is_shifted_char(ch: char) -> bool {
        matches!(
            ch,
            '!' | '@' | '#' | '$' | '%' | '^' | '&' | '*' | '(' | ')' | '_' | '+' | '{' | '}'
                | '|' | ':' | '"' | '<' | '>' | '?' | '~'
        )
    }

    fn char_to_keycode(ch: char) -> Option<u16> {
        let lower = ch.to_ascii_lowercase();
        Some(match lower {
            'a' => 0, 'b' => 11, 'c' => 8, 'd' => 2, 'e' => 14, 'f' => 3,
            'g' => 5, 'h' => 4, 'i' => 34, 'j' => 38, 'k' => 40, 'l' => 37,
            'm' => 46, 'n' => 45, 'o' => 31, 'p' => 35, 'q' => 12, 'r' => 15,
            's' => 1, 't' => 17, 'u' => 32, 'v' => 9, 'w' => 13, 'x' => 7,
            'y' => 16, 'z' => 6,
            '0' | ')' => 29, '1' | '!' => 18, '2' | '@' => 19, '3' | '#' => 20,
            '4' | '$' => 21, '5' | '%' => 23, '6' | '^' => 22, '7' | '&' => 26,
            '8' | '*' => 28, '9' | '(' => 25,
            ' ' => 49,
            '-' | '_' => 27,
            '=' | '+' => 24,
            '[' | '{' => 33,
            ']' | '}' => 30,
            '\\' | '|' => 42,
            ';' | ':' => 41,
            '\'' | '"' => 39,
            ',' | '<' => 43,
            '.' | '>' => 47,
            '/' | '?' => 44,
            '`' | '~' => 50,
            '\t' => 48,
            _ => return None,
        })
    }

    fn key_name_to_code(key: &str) -> Result<u16, AdapterError> {
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
            "forwarddelete" => 117,
            "home" => 115,
            "end" => 119,
            "pageup" => 116,
            "pagedown" => 121,
            "left" => 123, "right" => 124, "down" => 125, "up" => 126,
            "f1" => 122, "f2" => 120, "f3" => 99, "f4" => 118,
            "f5" => 96, "f6" => 97, "f7" => 98, "f8" => 100,
            "f9" => 101, "f10" => 109, "f11" => 103, "f12" => 111,
            other => {
                return Err(AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    format!("Unknown key: '{other}'"),
                ))
            }
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
