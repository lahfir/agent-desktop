use std::collections::HashMap;

use super::*;

const KEYSYM_A: u32 = 0x61;
const KEYSYM_UPPER_A: u32 = 0x41;
const KEYSYM_ONE: u32 = 0x31;
const KEYSYM_EXCLAM: u32 = 0x21;
const KEYSYM_TWO: u32 = 0x32;
const KEYSYM_AT: u32 = 0x40;
const KEYSYM_E_ACUTE: u32 = 0xe9;
const KEYSYM_RETURN: u32 = 0xff0d;

const KEYMAP: &str = r#"xkb_keymap {
xkb_keycodes "test" {
    minimum = 8;
    maximum = 255;
    <AE01> = 10;
    <AE02> = 11;
    <RTRN> = 36;
    <AC01> = 38;
    <LFSH> = 50;
};
xkb_types "test" {
    virtual_modifiers LevelThree;
    type "ONE_LEVEL" {
        modifiers = none;
        level_name[Level1] = "Any";
    };
    type "TWO_LEVEL" {
        modifiers = Shift;
        map[Shift] = Level2;
        level_name[Level1] = "Base";
        level_name[Level2] = "Shift";
    };
    type "ALPHABETIC" {
        modifiers = Shift+Lock;
        map[Shift] = Level2;
        map[Lock] = Level2;
        level_name[Level1] = "Base";
        level_name[Level2] = "Caps";
    };
    type "THREE_LEVEL" {
        modifiers = Shift+Mod5;
        map[Shift] = Level2;
        map[Mod5] = Level3;
        level_name[Level1] = "Base";
        level_name[Level2] = "Shift";
        level_name[Level3] = "Alt Base";
    };
};
xkb_compatibility "test" {
    interpret Shift_L {
        action = SetMods(modifiers=Shift);
    };
};
xkb_symbols "test" {
    key <AE01> { [ 1, exclam ] };
    key <AE02> { type = "THREE_LEVEL", [ 2, at, eacute ] };
    key <RTRN> { [ Return ] };
    key <AC01> { type = "ALPHABETIC", [ a, A ] };
    key <LFSH> { [ Shift_L ] };
    modifier_map Shift { <LFSH> };
};
};
"#;

fn sample() -> KeyTable {
    KeyTable::from_keys(HashMap::from([
        (
            KEYSYM_SHIFT_L,
            EvdevKey {
                code: 42,
                shift: false,
            },
        ),
        (
            KEYSYM_A,
            EvdevKey {
                code: 30,
                shift: false,
            },
        ),
        (
            KEYSYM_UPPER_A,
            EvdevKey {
                code: 30,
                shift: true,
            },
        ),
    ]))
}

#[test]
fn keymap_text_maps_base_and_shift_levels_to_evdev_codes() {
    let mut text = KEYMAP.as_bytes().to_vec();
    text.push(0);
    let table = KeyTable::from_keymap(&text, 0).expect("the test keymap compiles");
    let key = |code, shift| Some(EvdevKey { code, shift });
    assert_eq!(table.lookup(KEYSYM_A), key(30, false));
    assert_eq!(table.lookup(KEYSYM_UPPER_A), key(30, true));
    assert_eq!(table.lookup(KEYSYM_ONE), key(2, false));
    assert_eq!(table.lookup(KEYSYM_EXCLAM), key(2, true));
    assert_eq!(table.lookup(KEYSYM_TWO), key(3, false));
    assert_eq!(table.lookup(KEYSYM_AT), key(3, true));
    assert_eq!(table.lookup(KEYSYM_RETURN), key(28, false));
    assert_eq!(table.lookup(KEYSYM_SHIFT_L), key(42, false));
}

#[test]
fn keymap_text_leaves_out_keysyms_beyond_the_shift_level() {
    let table = KeyTable::from_keymap(KEYMAP.as_bytes(), 0).expect("the test keymap compiles");
    assert_eq!(table.lookup(KEYSYM_E_ACUTE), None);
}

#[test]
fn a_shift_level_keysym_is_wrapped_in_its_own_shift_press() {
    let table = sample();
    let mut state = PressState::default();
    assert_eq!(
        table.plan(KEYSYM_UPPER_A, true, &mut state),
        Some(vec![(42, true), (30, true)])
    );
    assert_eq!(
        table.plan(KEYSYM_UPPER_A, false, &mut state),
        Some(vec![(30, false), (42, false)])
    );
    assert_eq!(state, PressState::default());
}

#[test]
fn a_shift_level_keysym_under_a_held_shift_is_sent_alone() {
    let table = sample();
    let mut state = PressState::default();
    assert_eq!(
        table.plan(KEYSYM_SHIFT_L, true, &mut state),
        Some(vec![(42, true)])
    );
    assert_eq!(
        table.plan(KEYSYM_UPPER_A, true, &mut state),
        Some(vec![(30, true)])
    );
    assert_eq!(
        table.plan(KEYSYM_UPPER_A, false, &mut state),
        Some(vec![(30, false)])
    );
    assert_eq!(
        table.plan(KEYSYM_SHIFT_L, false, &mut state),
        Some(vec![(42, false)])
    );
    assert_eq!(state, PressState::default());
}

#[test]
fn a_keysym_outside_the_table_has_no_plan() {
    let mut state = PressState::default();
    assert_eq!(sample().plan(KEYSYM_E_ACUTE, true, &mut state), None);
}
