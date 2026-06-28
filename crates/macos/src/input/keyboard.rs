use agent_desktop_core::{
    action::KeyCombo,
    error::{AdapterError, ErrorCode},
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use accessibility_sys::{
        AXUIElementCreateSystemWide, AXUIElementPostKeyboardEvent, kAXErrorCannotComplete,
        kAXErrorSuccess,
    };
    use std::time::Duration;

    pub fn synthesize_key(combo: &KeyCombo) -> Result<(), AdapterError> {
        tracing::debug!(
            "keyboard: synthesize_key {}{}",
            if combo.modifiers.is_empty() {
                String::new()
            } else {
                format!(
                    "{}+",
                    combo
                        .modifiers
                        .iter()
                        .map(|m| format!("{m:?}"))
                        .collect::<Vec<_>>()
                        .join("+")
                )
            },
            combo.key
        );
        let key_code = key_name_to_code(&combo.key)?;

        let sys_wide = unsafe { AXUIElementCreateSystemWide() };
        if sys_wide.is_null() {
            return Err(AdapterError::internal(
                "Failed to create system-wide AX element",
            ));
        }

        let mut pressed_mods = Vec::new();
        for m in &combo.modifiers {
            let mod_code = modifier_keycode(m);
            let err = unsafe { AXUIElementPostKeyboardEvent(sys_wide, 0, mod_code, true) };
            if err != kAXErrorSuccess {
                release_modifiers(sys_wide, &pressed_mods);
                unsafe { core_foundation::base::CFRelease(sys_wide as _) };
                return Err(AdapterError::internal(format!(
                    "AXUIElementPostKeyboardEvent modifier-down failed (err={err})"
                )));
            }
            pressed_mods.push(m.clone());
        }

        let err_down = post_event(sys_wide, key_code, true);
        let err_up = post_event(sys_wide, key_code, false);

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

    pub fn synthesize_key_state(combo: &KeyCombo, down: bool) -> Result<(), AdapterError> {
        tracing::debug!(
            "keyboard: synthesize_key_state key={} down={down}",
            combo.key
        );
        let key_code = key_name_to_code(&combo.key)?;
        let sys_wide = unsafe { AXUIElementCreateSystemWide() };
        if sys_wide.is_null() {
            return Err(AdapterError::internal(
                "Failed to create system-wide AX element",
            ));
        }

        let result = (|| {
            if down {
                let mut pressed_mods = Vec::new();
                for m in &combo.modifiers {
                    if let Err(err) = post_checked(sys_wide, modifier_keycode(m), true, 0, 1) {
                        release_modifiers(sys_wide, &pressed_mods);
                        return Err(err);
                    }
                    pressed_mods.push(m.clone());
                }
                if let Err(err) = post_checked(sys_wide, key_code, true, 0, 1) {
                    release_modifiers(sys_wide, &pressed_mods);
                    return Err(err);
                }
                Ok(())
            } else {
                let key_result = post_checked(sys_wide, key_code, false, 0, 1);
                let mut first_mod_err: Option<AdapterError> = None;
                for m in combo.modifiers.iter().rev() {
                    if let Err(err) = post_checked(sys_wide, modifier_keycode(m), false, 0, 1) {
                        if first_mod_err.is_none() {
                            first_mod_err = Some(err);
                        }
                    }
                }
                key_result.and(first_mod_err.map_or(Ok(()), Err))
            }
        })();

        unsafe { core_foundation::base::CFRelease(sys_wide as _) };
        result
    }

    pub fn synthesize_text(text: &str) -> Result<(), AdapterError> {
        tracing::debug!("keyboard: synthesize_text {} chars", text.chars().count());
        let sys_wide = unsafe { AXUIElementCreateSystemWide() };
        if sys_wide.is_null() {
            return Err(AdapterError::internal(
                "Failed to create system-wide AX element",
            ));
        }

        let total = text.chars().count();
        let mut delivered = 0usize;
        let result = (|| {
            for ch in text.chars() {
                if ch == '\n' {
                    post_char_key(sys_wide, 36, delivered, total)?;
                } else if let Some(code) = char_to_keycode(ch) {
                    let needs_shift = ch.is_ascii_uppercase() || is_shifted_char(ch);
                    if needs_shift {
                        post_checked(sys_wide, 56, true, delivered, total)?;
                    }
                    let char_result = post_char_key(sys_wide, code, delivered, total);
                    if needs_shift {
                        let shift_up = post_checked(sys_wide, 56, false, delivered, total);
                        char_result.and(shift_up)?;
                    } else {
                        char_result?;
                    }
                } else {
                    return Err(AdapterError::new(
                        ErrorCode::ActionNotSupported,
                        format!("Cannot synthesize character '{ch}' with keyboard fallback"),
                    )
                    .with_suggestion("Use set-value for non-ASCII text when supported."));
                }
                delivered += 1;
                std::thread::sleep(Duration::from_millis(4));
            }
            Ok(())
        })();

        unsafe { core_foundation::base::CFRelease(sys_wide as _) };
        result
    }

    pub fn synthesize_keycode(key_code: u16, repeats: u32) -> Result<(), AdapterError> {
        let sys_wide = unsafe { AXUIElementCreateSystemWide() };
        if sys_wide.is_null() {
            return Err(AdapterError::internal(
                "Failed to create system-wide AX element",
            ));
        }
        let result = (|| {
            for i in 0..repeats {
                post_char_key(sys_wide, key_code, i as usize, repeats as usize)?;
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(())
        })();
        unsafe { core_foundation::base::CFRelease(sys_wide as _) };
        result
    }

    pub fn synthesize_key_for_element(
        el: &crate::tree::AXElement,
        combo: &KeyCombo,
    ) -> Result<(), AdapterError> {
        let key_code = key_name_to_code(&combo.key)?;
        let mut pressed = Vec::new();
        for m in &combo.modifiers {
            if let Err(err) = post_checked(el.0, modifier_keycode(m), true, 0, 1) {
                release_modifiers(el.0, &pressed);
                return Err(err);
            }
            pressed.push(m.clone());
        }
        let key_result = post_char_key(el.0, key_code, 0, 1);
        let mut release_result = Ok(());
        for m in pressed.iter().rev() {
            if let Err(err) = post_checked(el.0, modifier_keycode(m), false, 0, 1) {
                if release_result.is_ok() {
                    release_result = Err(err);
                }
            }
        }
        if key_result.is_err() {
            release_key_system_wide(key_code);
        }
        if key_result.is_err() || release_result.is_err() {
            release_modifiers_system_wide(&pressed);
        }
        key_result.and(release_result)
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

    fn release_modifiers_system_wide(modifiers: &[agent_desktop_core::action::Modifier]) {
        let sys_wide = unsafe { AXUIElementCreateSystemWide() };
        if sys_wide.is_null() {
            return;
        }
        release_modifiers(sys_wide, modifiers);
        unsafe { core_foundation::base::CFRelease(sys_wide as _) };
    }

    fn release_key_system_wide(key_code: u16) {
        let sys_wide = unsafe { AXUIElementCreateSystemWide() };
        if sys_wide.is_null() {
            return;
        }
        unsafe { AXUIElementPostKeyboardEvent(sys_wide, 0, key_code, false) };
        unsafe { core_foundation::base::CFRelease(sys_wide as _) };
    }

    fn post_char_key(
        el: accessibility_sys::AXUIElementRef,
        code: u16,
        delivered: usize,
        total: usize,
    ) -> Result<(), AdapterError> {
        post_checked(el, code, true, delivered, total)?;
        post_checked(el, code, false, delivered, total)
    }

    fn post_checked(
        el: accessibility_sys::AXUIElementRef,
        code: u16,
        down: bool,
        delivered: usize,
        total: usize,
    ) -> Result<(), AdapterError> {
        let err = post_event(el, code, down);
        if err == kAXErrorSuccess {
            return Ok(());
        }
        if err == kAXErrorCannotComplete {
            std::thread::sleep(Duration::from_millis(10));
            let retry = post_event(el, code, down);
            if retry == kAXErrorSuccess {
                return Ok(());
            }
            return Err(post_error(retry, delivered, total));
        }
        Err(post_error(err, delivered, total))
    }

    fn post_event(el: accessibility_sys::AXUIElementRef, code: u16, down: bool) -> i32 {
        unsafe { AXUIElementPostKeyboardEvent(el, 0, code, down) }
    }

    fn post_error(err: i32, delivered: usize, total: usize) -> AdapterError {
        AdapterError::new(
            ErrorCode::ActionFailed,
            format!(
                "Keyboard synthesis failed after {delivered}/{total} characters delivered (err={err})"
            ),
        )
        .with_suggestion("Retry after refreshing the target snapshot and confirming focus.")
    }

    fn modifier_keycode(m: &agent_desktop_core::action::Modifier) -> u16 {
        crate::input::keyboard_map::modifier_keycode(m)
    }

    fn is_shifted_char(ch: char) -> bool {
        crate::input::keyboard_map::is_shifted_char(ch)
    }

    fn char_to_keycode(ch: char) -> Option<u16> {
        crate::input::keyboard_map::char_to_keycode(ch)
    }

    fn key_name_to_code(key: &str) -> Result<u16, AdapterError> {
        crate::input::keyboard_map::key_name_to_code(key)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn synthesize_key(_combo: &KeyCombo) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("synthesize_key"))
    }

    pub fn synthesize_key_state(_combo: &KeyCombo, _down: bool) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("synthesize_key_state"))
    }

    pub fn synthesize_text(_text: &str) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("synthesize_text"))
    }

    pub fn synthesize_keycode(_key_code: u16, _repeats: u32) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("synthesize_keycode"))
    }

    pub fn synthesize_key_for_element(
        _el: &crate::tree::AXElement,
        _combo: &KeyCombo,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("synthesize_key_for_element"))
    }
}

pub use imp::{
    synthesize_key, synthesize_key_for_element, synthesize_key_state, synthesize_keycode,
    synthesize_text,
};
