//! Key-name to Windows virtual-key resolution.
//!
//! Key *names* are a shared contract with macOS: every name
//! `crates/macos/src/input/keyboard_map.rs` accepts must resolve here too
//! (KTD10), even though what a combo built from those names *does* is not
//! portable. `delete`/`backspace` and `forwarddelete` keep the same split
//! macOS draws between its single Backspace-shaped key and the separate
//! forward-delete key, mapped onto Windows' own two distinct keys
//! (`VK_BACK`/`VK_DELETE`) rather than collapsing them.
//!
//! Single-character names (`a`-`z`, `0`-`9`) resolve through `VkKeyScanW`
//! so the physical key pressed matches the active keyboard layout, falling
//! back to the fixed ASCII-derived virtual-key code (numerically identical
//! to `VkKeyScanW`'s answer on a US layout) when the layout has no key that
//! produces the requested character at all.

use agent_desktop_core::{AdapterError, ErrorCode};

pub(crate) const VK_BACK: u16 = 0x08;
pub(crate) const VK_TAB: u16 = 0x09;
pub(crate) const VK_RETURN: u16 = 0x0D;
pub(crate) const VK_SHIFT: u16 = 0x10;
pub(crate) const VK_CONTROL: u16 = 0x11;
pub(crate) const VK_MENU: u16 = 0x12;
pub(crate) const VK_ESCAPE: u16 = 0x1B;
pub(crate) const VK_SPACE: u16 = 0x20;
pub(crate) const VK_PRIOR: u16 = 0x21;
pub(crate) const VK_NEXT: u16 = 0x22;
pub(crate) const VK_END: u16 = 0x23;
pub(crate) const VK_HOME: u16 = 0x24;
pub(crate) const VK_LEFT: u16 = 0x25;
pub(crate) const VK_UP: u16 = 0x26;
pub(crate) const VK_RIGHT: u16 = 0x27;
pub(crate) const VK_DOWN: u16 = 0x28;
pub(crate) const VK_DELETE: u16 = 0x2E;
pub(crate) const VK_LWIN: u16 = 0x5B;
const VK_F1: u16 = 0x70;
const VK_F2: u16 = 0x71;
const VK_F3: u16 = 0x72;
const VK_F4: u16 = 0x73;
const VK_F5: u16 = 0x74;
const VK_F6: u16 = 0x75;
const VK_F7: u16 = 0x76;
const VK_F8: u16 = 0x77;
const VK_F9: u16 = 0x78;
const VK_F10: u16 = 0x79;
const VK_F11: u16 = 0x7A;
const VK_F12: u16 = 0x7B;

#[cfg(target_os = "windows")]
const _: () = {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse as win32;
    assert!(VK_BACK == win32::VK_BACK);
    assert!(VK_TAB == win32::VK_TAB);
    assert!(VK_RETURN == win32::VK_RETURN);
    assert!(VK_SHIFT == win32::VK_SHIFT);
    assert!(VK_CONTROL == win32::VK_CONTROL);
    assert!(VK_MENU == win32::VK_MENU);
    assert!(VK_ESCAPE == win32::VK_ESCAPE);
    assert!(VK_SPACE == win32::VK_SPACE);
    assert!(VK_PRIOR == win32::VK_PRIOR);
    assert!(VK_NEXT == win32::VK_NEXT);
    assert!(VK_END == win32::VK_END);
    assert!(VK_HOME == win32::VK_HOME);
    assert!(VK_LEFT == win32::VK_LEFT);
    assert!(VK_UP == win32::VK_UP);
    assert!(VK_RIGHT == win32::VK_RIGHT);
    assert!(VK_DOWN == win32::VK_DOWN);
    assert!(VK_DELETE == win32::VK_DELETE);
    assert!(VK_LWIN == win32::VK_LWIN);
    assert!(VK_F1 == win32::VK_F1);
    assert!(VK_F12 == win32::VK_F12);
};

pub(crate) fn key_name_to_vk(key: &str) -> Result<u16, AdapterError> {
    if let Some(vk) = named_key_vk(key) {
        return Ok(vk);
    }
    if let Some(vk) = character_key_vk(key) {
        return Ok(vk);
    }
    Err(unknown_key_error(key))
}

fn named_key_vk(key: &str) -> Option<u16> {
    let vk = match key {
        "return" | "enter" => VK_RETURN,
        "escape" | "esc" => VK_ESCAPE,
        "tab" => VK_TAB,
        "space" => VK_SPACE,
        "delete" | "backspace" => VK_BACK,
        "forwarddelete" => VK_DELETE,
        "home" => VK_HOME,
        "end" => VK_END,
        "pageup" => VK_PRIOR,
        "pagedown" => VK_NEXT,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "down" => VK_DOWN,
        "up" => VK_UP,
        "cmd" | "command" => VK_LWIN,
        "shift" => VK_SHIFT,
        "alt" | "option" => VK_MENU,
        "ctrl" | "control" => VK_CONTROL,
        "f1" => VK_F1,
        "f2" => VK_F2,
        "f3" => VK_F3,
        "f4" => VK_F4,
        "f5" => VK_F5,
        "f6" => VK_F6,
        "f7" => VK_F7,
        "f8" => VK_F8,
        "f9" => VK_F9,
        "f10" => VK_F10,
        "f11" => VK_F11,
        "f12" => VK_F12,
        _ => return None,
    };
    Some(vk)
}

fn character_key_vk(key: &str) -> Option<u16> {
    let mut chars = key.chars();
    let character = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if !(character.is_ascii_lowercase() || character.is_ascii_digit()) {
        return None;
    }
    Some(resolve_character_vk(character))
}

fn resolve_character_vk(character: char) -> u16 {
    imp::vk_key_scan(character).unwrap_or_else(|| fallback_ascii_vk(character))
}

fn fallback_ascii_vk(character: char) -> u16 {
    character.to_ascii_uppercase() as u16
}

fn unknown_key_error(key: &str) -> AdapterError {
    AdapterError::new(ErrorCode::InvalidArgs, format!("Unknown key: '{key}'")).with_suggestion(
        "Valid keys: a-z, 0-9, return, escape, tab, space, delete, forwarddelete, home, end, \
         pageup, pagedown, left, right, up, down, cmd, shift, alt, ctrl, f1-f12",
    )
}

#[cfg(target_os = "windows")]
mod imp {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::VkKeyScanW;

    pub(super) fn vk_key_scan(character: char) -> Option<u16> {
        let code_unit = character as u16;
        let result = unsafe { VkKeyScanW(code_unit) };
        if result == -1 {
            return None;
        }
        Some((result as u16) & 0x00FF)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub(super) fn vk_key_scan(_character: char) -> Option<u16> {
        None
    }
}

#[cfg(test)]
#[path = "keyboard_map_tests.rs"]
mod tests;
