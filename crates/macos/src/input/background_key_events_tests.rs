use super::*;
use agent_desktop_core::Modifier;
use std::ffi::c_void;

unsafe extern "C" {
    fn CGEventKeyboardGetUnicodeString(
        event: *const c_void,
        max_length: libc::c_ulong,
        actual_length: *mut libc::c_ulong,
        buffer: *mut u16,
    );
}

fn event_text(event: &CGEvent) -> String {
    use foreign_types::ForeignType;

    let mut buffer = [0_u16; 8];
    let mut length: libc::c_ulong = 0;
    unsafe {
        CGEventKeyboardGetUnicodeString(
            event.as_ptr().cast(),
            buffer.len() as libc::c_ulong,
            &mut length,
            buffer.as_mut_ptr(),
        );
    }
    String::from_utf16(&buffer[..length as usize]).unwrap()
}

fn text_of(planned: &PlannedKey) -> String {
    String::from_utf16(&planned.text).unwrap()
}

fn routing(route: bool) -> KeyRouting {
    KeyRouting {
        pid: 4242,
        window_number: 15592,
        route,
    }
}

#[test]
fn combo_is_one_down_up_pair_with_modifier_flags_on_both() {
    let planned = plan(&BackgroundKeyInput::Combo(KeyCombo {
        key: "s".into(),
        modifiers: vec![Modifier::Meta, Modifier::Shift],
    }))
    .unwrap();

    let flags = CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagShift;
    assert_eq!(planned.len(), 2);
    assert_eq!((planned[0].key_code, planned[0].down), (1, true));
    assert_eq!((planned[1].key_code, planned[1].down), (1, false));
    assert!(planned.iter().all(|key| key.flags == flags));
    assert!(planned.iter().all(|key| key.text.is_empty()));
    assert_eq!(planned[0].pause_after, COMBO_HOLD);
}

#[test]
fn unknown_combo_key_fails_before_anything_is_built() {
    let error = plan(&BackgroundKeyInput::Combo(KeyCombo {
        key: "hyperkey".into(),
        modifiers: Vec::new(),
    }))
    .unwrap_err();

    assert_eq!(error.code, agent_desktop_core::ErrorCode::InvalidArgs);
}

#[test]
fn text_is_one_paced_pair_per_character_in_order() {
    let text = "hello from background, 42!";
    let planned = plan(&BackgroundKeyInput::Text(text.into())).unwrap();

    assert_eq!(planned.len(), text.chars().count() * 2);
    let typed: String = planned.iter().filter(|key| key.down).map(text_of).collect();
    assert_eq!(typed, text);
    for pair in planned.chunks(2) {
        assert!(pair[0].down && !pair[1].down);
        assert_eq!(pair[0].text, pair[1].text);
        assert_eq!(pair[0].key_code, pair[1].key_code);
        assert_eq!(pair[0].pause_after, TEXT_HOLD);
        assert_eq!(pair[1].pause_after, TEXT_GAP);
    }
}

#[test]
fn text_rides_on_one_key_code_except_return_and_tab() {
    let planned = plan(&BackgroundKeyInput::Text("hH 4,č😀\r\n\t".into())).unwrap();
    let downs: Vec<(u16, String)> = planned
        .iter()
        .filter(|key| key.down)
        .map(|key| (key.key_code, text_of(key)))
        .collect();

    assert!(planned.iter().all(|key| key.flags.is_empty()));
    assert_eq!(
        downs,
        [
            (TEXT_KEY, "h".to_string()),
            (TEXT_KEY, "H".to_string()),
            (TEXT_KEY, " ".to_string()),
            (TEXT_KEY, "4".to_string()),
            (TEXT_KEY, ",".to_string()),
            (TEXT_KEY, "č".to_string()),
            (TEXT_KEY, "😀".to_string()),
            (RETURN_KEY, "\r".to_string()),
            (TAB_KEY, "\t".to_string()),
        ]
    );
}

#[test]
fn built_events_carry_key_code_flags_and_unicode_text() {
    let planned = plan(&BackgroundKeyInput::Text("Ž".into())).unwrap();
    let down = build(&planned[0], &routing(false)).unwrap();
    let up = build(&planned[1], &routing(false)).unwrap();

    assert_eq!(
        down.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
        i64::from(TEXT_KEY)
    );
    assert_eq!(event_text(&down), "Ž");
    assert_eq!(event_text(&up), "Ž");
    assert_eq!(
        down.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
        0
    );
    assert_eq!(down.get_integer_value_field(FIELD_WINDOW_NUMBER), 0);
}

#[test]
fn route_layer_names_the_target_process_and_window() {
    let planned = plan(&BackgroundKeyInput::Combo(KeyCombo {
        key: "enter".into(),
        modifiers: vec![Modifier::Ctrl],
    }))
    .unwrap();
    let event = build(&planned[0], &routing(true)).unwrap();

    assert_eq!(
        event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
        i64::from(RETURN_KEY)
    );
    assert!(event.get_flags().contains(CGEventFlags::CGEventFlagControl));
    assert_eq!(
        event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
        4242
    );
    assert_eq!(event.get_integer_value_field(FIELD_WINDOW_NUMBER), 15592);
}

#[test]
fn authenticating_a_built_key_event_is_safe_on_this_os() {
    let planned = plan(&BackgroundKeyInput::Text("a".into())).unwrap();
    let event = build(&planned[0], &routing(true)).unwrap();

    let _ = crate::input::skylight::authenticate(&event, std::process::id() as libc::pid_t);
    assert_eq!(event_text(&event), "a");
}

/// `press enter --background` once typed no newline in VS Code while a typed
/// `\n` did. The only construction difference was the Unicode string: the
/// combo posted Return with none, so AppKit handed Chromium a key down with
/// empty `characters`. Return and Tab combos now carry the same text as the
/// typed character.
#[test]
fn return_and_tab_combos_build_the_same_events_as_typed_line_breaks_and_tabs() {
    for (key, typed) in [("enter", "\n"), ("return", "\n"), ("tab", "\t")] {
        let combo = plan(&BackgroundKeyInput::Combo(KeyCombo {
            key: key.into(),
            modifiers: Vec::new(),
        }))
        .unwrap();
        let text = plan(&BackgroundKeyInput::Text(typed.into())).unwrap();

        let shape = |keys: &[PlannedKey]| {
            keys.iter()
                .map(|planned| {
                    (
                        planned.key_code,
                        planned.down,
                        planned.flags,
                        planned.text.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(shape(&combo), shape(&text), "{key}");
    }
}
