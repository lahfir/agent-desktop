//! Holds modifier VKs down around a mouse click sequence.
//!
//! A full chord primitive (release-guard sweep against live key state,
//! blocked-combo canonicalization) belongs to keyboard synthesis; this is
//! the minimal press/release a mouse click needs so `Click` with modifiers
//! works without depending on that primitive. `ModifierGuard::drop` always
//! posts the release, so an aborted click sequence cannot leave a modifier
//! held - best-effort, like every other physical post here: the release is
//! sent, never verified.

use agent_desktop_core::Modifier;

const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_LWIN: u16 = 0x5B;

#[cfg(target_os = "windows")]
const _: () = {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse as win32;
    assert!(VK_SHIFT == win32::VK_SHIFT);
    assert!(VK_CONTROL == win32::VK_CONTROL);
    assert!(VK_MENU == win32::VK_MENU);
    assert!(VK_LWIN == win32::VK_LWIN);
};

fn modifier_vk(modifier: &Modifier) -> u16 {
    match modifier {
        Modifier::Meta => VK_LWIN,
        Modifier::Ctrl => VK_CONTROL,
        Modifier::Alt => VK_MENU,
        Modifier::Shift => VK_SHIFT,
    }
}

pub(crate) struct ModifierGuard {
    held: Vec<u16>,
    released: bool,
}

impl ModifierGuard {
    pub(crate) fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        for vk in self.held.iter().rev().copied() {
            imp::post_key(vk, true);
        }
    }
}

impl Drop for ModifierGuard {
    fn drop(&mut self) {
        self.release();
    }
}

pub(crate) fn press_modifiers(modifiers: &[Modifier]) -> ModifierGuard {
    let held: Vec<u16> = modifiers.iter().map(modifier_vk).collect();
    for &vk in &held {
        imp::post_key(vk, false);
    }
    ModifierGuard {
        held,
        released: false,
    }
}

#[cfg(target_os = "windows")]
mod imp {
    pub(super) fn post_key(vk: u16, key_up: bool) {
        #[cfg(test)]
        {
            super::modifier_fake_sink::record(vk, key_up);
        }
        #[cfg(not(test))]
        {
            native::send(vk, key_up);
        }
    }

    #[cfg(not(test))]
    mod native {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        };

        pub(super) fn send(vk: u16, key_up: bool) {
            let input = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        wScan: 0,
                        dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            unsafe {
                SendInput(1, &input, core::mem::size_of::<INPUT>() as i32);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub(super) fn post_key(_vk: u16, _key_up: bool) {}
}

#[cfg(all(test, target_os = "windows"))]
pub(crate) mod modifier_fake_sink {
    use std::cell::RefCell;

    thread_local! {
        static RECORDED: RefCell<Vec<(u16, bool)>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn reset() {
        RECORDED.with(|cell| cell.borrow_mut().clear());
    }

    pub(crate) fn recorded() -> Vec<(u16, bool)> {
        RECORDED.with(|cell| cell.borrow().clone())
    }

    pub(super) fn record(vk: u16, key_up: bool) {
        RECORDED.with(|cell| cell.borrow_mut().push((vk, key_up)));
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "mouse_modifier_tests.rs"]
mod tests;
