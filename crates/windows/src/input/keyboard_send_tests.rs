use super::key_state_fake_sink as key_state;
use super::keyboard_send_fake_sink as sink;
use super::{KeyboardInputEvent, key_is_down, post_keyboard_inputs};

fn event(vk: u16) -> KeyboardInputEvent {
    KeyboardInputEvent {
        vk,
        scan: 0,
        flags: 0,
    }
}

#[test]
fn posted_events_are_recorded_in_the_order_they_were_sent() {
    sink::reset();

    post_keyboard_inputs(&[event(1), event(2)]);
    post_keyboard_inputs(&[event(3)]);

    assert_eq!(sink::recorded(), vec![event(1), event(2), event(3)]);
}

#[test]
fn an_empty_batch_records_nothing() {
    sink::reset();

    post_keyboard_inputs(&[]);

    assert!(sink::recorded().is_empty());
}

#[test]
fn key_state_fake_defaults_every_key_to_not_down() {
    key_state::reset();

    assert!(!key_is_down(0x11));
}

#[test]
fn key_state_fake_reports_down_only_for_keys_marked_down() {
    key_state::reset();
    key_state::set_down(0x11);

    assert!(key_is_down(0x11));
    assert!(!key_is_down(0x10));
}
