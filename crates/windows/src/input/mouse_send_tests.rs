use super::mouse_send_fake_sink as sink;
use super::{MouseInputEvent, post_mouse_inputs};

fn event(flags: u32) -> MouseInputEvent {
    MouseInputEvent {
        dx: 0,
        dy: 0,
        mouse_data: 0,
        flags,
    }
}

#[test]
fn posted_events_are_recorded_in_the_order_they_were_sent() {
    sink::reset();

    post_mouse_inputs(&[event(1), event(2)]);
    post_mouse_inputs(&[event(3)]);

    let recorded = sink::recorded();
    assert_eq!(
        recorded,
        vec![event(1), event(2), event(3)],
        "the fake sink must preserve post order, not just the total count"
    );
}

#[test]
fn an_empty_batch_records_nothing() {
    sink::reset();

    post_mouse_inputs(&[]);

    assert!(sink::recorded().is_empty());
}
