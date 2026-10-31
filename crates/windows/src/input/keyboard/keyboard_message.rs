//! Window-message plans for typing into a Chromium window that is not the
//! foreground window.
//!
//! Chromium drops `WM_CHAR` while its top-level window is inactive, so the
//! plan brackets the text with the activation messages Windows itself sends
//! on activation: `WM_NCACTIVATE`, `WM_ACTIVATE` and `WM_SETFOCUS`, then
//! their inactive counterparts. These change what the window believes about
//! its own activation state; the desktop foreground window never changes.
//! Each UTF-16 unit is a `VK_PACKET` down, the `WM_CHAR`, and a `VK_PACKET`
//! up, which is what `SendInput` with `KEYEVENTF_UNICODE` produces after
//! `TranslateMessage`. A line break is a `VK_RETURN` down, a CR `WM_CHAR`
//! and a `VK_RETURN` up. The messages are sent rather than posted, so no
//! message loop runs `TranslateMessage` on them and each `WM_CHAR` is the
//! only character the window sees. A CR LF pair is one line break.

use crate::input::keyboard::keyboard_text::typed_chars;

const WM_ACTIVATE: u32 = 0x0006;
const WM_SETFOCUS: u32 = 0x0007;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_NCACTIVATE: u32 = 0x0086;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_CHAR: u32 = 0x0102;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WA_ACTIVE: usize = 1;
const WA_INACTIVE: usize = 0;
const MK_LBUTTON: usize = 0x0001;
const VK_RETURN: usize = 0x0D;
const VK_PACKET: usize = 0xE7;
const CARRIAGE_RETURN: usize = 0x0D;
const REPEAT_ONE: isize = 0x0000_0001;
const RETURN_SCAN_CODE: isize = 0x001C_0000;
const KEY_UP_TRANSITION: isize = 0xC000_0000;

#[cfg(target_os = "windows")]
const _: () = {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse as keys;
    use windows_sys::Win32::UI::WindowsAndMessaging as msgs;
    assert!(WM_ACTIVATE == msgs::WM_ACTIVATE);
    assert!(WM_SETFOCUS == msgs::WM_SETFOCUS);
    assert!(WM_KILLFOCUS == msgs::WM_KILLFOCUS);
    assert!(WM_NCACTIVATE == msgs::WM_NCACTIVATE);
    assert!(WM_KEYDOWN == msgs::WM_KEYDOWN);
    assert!(WM_KEYUP == msgs::WM_KEYUP);
    assert!(WM_CHAR == msgs::WM_CHAR);
    assert!(WM_MOUSEMOVE == msgs::WM_MOUSEMOVE);
    assert!(WM_LBUTTONDOWN == msgs::WM_LBUTTONDOWN);
    assert!(WM_LBUTTONUP == msgs::WM_LBUTTONUP);
    assert!(WA_ACTIVE == msgs::WA_ACTIVE as usize);
    assert!(WA_INACTIVE == msgs::WA_INACTIVE as usize);
    assert!(VK_RETURN == keys::VK_RETURN as usize);
    assert!(VK_PACKET == keys::VK_PACKET as usize);
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowMessage {
    pub(crate) msg: u32,
    pub(crate) wparam: usize,
    pub(crate) lparam: isize,
}

impl WindowMessage {
    const fn new(msg: u32, wparam: usize, lparam: isize) -> Self {
        Self {
            msg,
            wparam,
            lparam,
        }
    }

    /// Whether this message carries part of the typed text, as opposed to
    /// activation or the focusing click.
    #[cfg(test)]
    pub(crate) fn carries_text(&self) -> bool {
        matches!(self.msg, WM_KEYDOWN | WM_KEYUP | WM_CHAR)
    }
}

pub(crate) fn activation_messages(active: bool) -> [WindowMessage; 3] {
    if active {
        [
            WindowMessage::new(WM_NCACTIVATE, 1, 0),
            WindowMessage::new(WM_ACTIVATE, WA_ACTIVE, 0),
            WindowMessage::new(WM_SETFOCUS, 0, 0),
        ]
    } else {
        [
            WindowMessage::new(WM_NCACTIVATE, 0, 0),
            WindowMessage::new(WM_ACTIVATE, WA_INACTIVE, 0),
            WindowMessage::new(WM_KILLFOCUS, 0, 0),
        ]
    }
}

/// A left click at a point in the window's client coordinates.
pub(crate) fn click_messages(x: i32, y: i32) -> [WindowMessage; 3] {
    let point = ((y as u16 as isize) << 16) | (x as u16 as isize);
    [
        WindowMessage::new(WM_MOUSEMOVE, 0, point),
        WindowMessage::new(WM_LBUTTONDOWN, MK_LBUTTON, point),
        WindowMessage::new(WM_LBUTTONUP, 0, point),
    ]
}

pub(crate) fn text_messages(text: &str) -> Vec<WindowMessage> {
    let mut messages = Vec::new();
    for character in typed_chars(text) {
        if matches!(character, '\r' | '\n') {
            let lparam = REPEAT_ONE | RETURN_SCAN_CODE;
            messages.push(WindowMessage::new(WM_KEYDOWN, VK_RETURN, lparam));
            messages.push(WindowMessage::new(WM_CHAR, CARRIAGE_RETURN, lparam));
            messages.push(WindowMessage::new(
                WM_KEYUP,
                VK_RETURN,
                lparam | KEY_UP_TRANSITION,
            ));
            continue;
        }
        let mut encoded = [0_u16; 2];
        for &unit in character.encode_utf16(&mut encoded).iter() {
            messages.push(WindowMessage::new(WM_KEYDOWN, VK_PACKET, REPEAT_ONE));
            messages.push(WindowMessage::new(WM_CHAR, usize::from(unit), REPEAT_ONE));
            messages.push(WindowMessage::new(
                WM_KEYUP,
                VK_PACKET,
                REPEAT_ONE | KEY_UP_TRANSITION,
            ));
        }
    }
    messages
}

#[cfg(test)]
#[path = "keyboard_message_tests.rs"]
mod tests;
