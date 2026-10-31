use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::{c_char, c_int, c_void};

use agent_desktop_core::AdapterError;

use super::evdev_key::EvdevKey;
use super::press_state::PressState;
use super::xkb_library::{XkbLibrary, unavailable};
use crate::input::keyboard::key_names::KEYSYM_SHIFT_L;

const KEYSYM_SHIFT_R: u32 = 0xffe2;
const EVDEV_OFFSET: u32 = 8;
const KEYMAP_FORMAT_TEXT_V1: c_int = 1;
const MAX_MASKS: usize = 16;

type ContextNew = unsafe extern "C" fn(c_int) -> *mut c_void;
type Unref = unsafe extern "C" fn(*mut c_void);
type KeymapNew =
    unsafe extern "C" fn(*mut c_void, *const c_char, usize, c_int, c_int) -> *mut c_void;
type Keycode = unsafe extern "C" fn(*mut c_void) -> u32;
type Layouts = unsafe extern "C" fn(*mut c_void, u32) -> u32;
type Levels = unsafe extern "C" fn(*mut c_void, u32, u32) -> u32;
type Syms = unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut *const u32) -> c_int;
type Masks = unsafe extern "C" fn(*mut c_void, u32, u32, u32, *mut u32, usize) -> usize;
type ModIndex = unsafe extern "C" fn(*mut c_void, *const c_char) -> u32;

/// Where each keysym of the compositor's keymap sits: the lowest key that
/// produces it with no modifier, else with Shift alone. A keysym reachable
/// only through another level (AltGr, a second layout) is absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct KeyTable {
    keys: HashMap<u32, EvdevKey>,
}

impl KeyTable {
    /// Reads an XKB text keymap as KWin sends it, for its layout `layout`.
    pub(crate) fn from_keymap(keymap: &[u8], layout: u32) -> Result<Self, AdapterError> {
        let text = keymap.strip_suffix(&[0]).unwrap_or(keymap);
        let xkb = XkbLibrary::open()?;
        let mut keys = HashMap::new();
        unsafe {
            let context_new: ContextNew = xkb.symbol(c"xkb_context_new")?;
            let context_unref: Unref = xkb.symbol(c"xkb_context_unref")?;
            let keymap_new: KeymapNew = xkb.symbol(c"xkb_keymap_new_from_buffer")?;
            let keymap_unref: Unref = xkb.symbol(c"xkb_keymap_unref")?;
            let min_keycode: Keycode = xkb.symbol(c"xkb_keymap_min_keycode")?;
            let max_keycode: Keycode = xkb.symbol(c"xkb_keymap_max_keycode")?;
            let layouts: Layouts = xkb.symbol(c"xkb_keymap_num_layouts_for_key")?;
            let levels: Levels = xkb.symbol(c"xkb_keymap_num_levels_for_key")?;
            let syms: Syms = xkb.symbol(c"xkb_keymap_key_get_syms_by_level")?;
            let masks: Masks = xkb.symbol(c"xkb_keymap_key_get_mods_for_level")?;
            let mod_index: ModIndex = xkb.symbol(c"xkb_keymap_mod_get_index")?;
            let context = context_new(0);
            if context.is_null() {
                return Err(unavailable("xkb_context_new failed"));
            }
            let map = keymap_new(
                context,
                text.as_ptr().cast(),
                text.len(),
                KEYMAP_FORMAT_TEXT_V1,
                0,
            );
            context_unref(context);
            if map.is_null() {
                return Err(unavailable("the compositor keymap did not compile"));
            }
            let shift_mask = 1_u32
                .checked_shl(mod_index(map, c"Shift".as_ptr()))
                .unwrap_or(0);
            for keycode in min_keycode(map).max(EVDEV_OFFSET)..=max_keycode(map) {
                let count = layouts(map, keycode);
                if count == 0 {
                    continue;
                }
                let group = layout % count;
                for level in 0..levels(map, keycode, group) {
                    let mut found = [0_u32; MAX_MASKS];
                    let written = masks(map, keycode, group, level, found.as_mut_ptr(), MAX_MASKS);
                    let found = &found[..written.min(MAX_MASKS)];
                    let shift = if found.contains(&0) {
                        false
                    } else if shift_mask != 0 && found.contains(&shift_mask) {
                        true
                    } else {
                        continue;
                    };
                    let mut first: *const u32 = std::ptr::null();
                    let count = syms(map, keycode, group, level, &raw mut first);
                    let count = usize::try_from(count).unwrap_or(0);
                    if first.is_null() || count == 0 {
                        continue;
                    }
                    let key = EvdevKey {
                        code: keycode - EVDEV_OFFSET,
                        shift,
                    };
                    for keysym in std::slice::from_raw_parts(first, count) {
                        match keys.entry(*keysym) {
                            Entry::Vacant(slot) => {
                                slot.insert(key);
                            }
                            Entry::Occupied(mut slot) if slot.get().shift && !shift => {
                                slot.insert(key);
                            }
                            Entry::Occupied(_) => {}
                        }
                    }
                }
            }
            keymap_unref(map);
        }
        Ok(Self { keys })
    }

    pub(crate) fn lookup(&self, keysym: u32) -> Option<EvdevKey> {
        self.keys.get(&keysym).copied()
    }

    /// The evdev presses that send `keysym` going `down` or up. A keysym on
    /// the Shift level goes inside a Shift press of its own unless the caller
    /// already holds Shift.
    pub(crate) fn plan(
        &self,
        keysym: u32,
        down: bool,
        state: &mut PressState,
    ) -> Option<Vec<(u32, bool)>> {
        let key = self.lookup(keysym)?;
        if matches!(keysym, KEYSYM_SHIFT_L | KEYSYM_SHIFT_R) {
            state.explicit_shift = if down {
                state.explicit_shift + 1
            } else {
                state.explicit_shift.saturating_sub(1)
            };
            return Some(vec![(key.code, down)]);
        }
        let shift = self.lookup(KEYSYM_SHIFT_L)?.code;
        if down {
            if key.shift && state.explicit_shift == 0 {
                state.wrapped.push(key.code);
                return Some(vec![(shift, true), (key.code, true)]);
            }
            return Some(vec![(key.code, true)]);
        }
        match state.wrapped.iter().rposition(|code| *code == key.code) {
            Some(at) => {
                state.wrapped.remove(at);
                Some(vec![(key.code, false), (shift, false)])
            }
            None => Some(vec![(key.code, false)]),
        }
    }
}

#[cfg(test)]
impl KeyTable {
    pub(crate) fn from_keys(keys: HashMap<u32, EvdevKey>) -> Self {
        Self { keys }
    }
}

#[cfg(test)]
#[path = "key_table_tests.rs"]
mod tests;
