use super::imp::{count_changed, is_absent_error, record_error};
use super::*;
use accessibility_sys::{
    kAXErrorAttributeUnsupported, kAXErrorCannotComplete, kAXErrorFailure, kAXErrorNoValue,
};

#[test]
fn unsupported_and_missing_child_attributes_are_authoritatively_absent() {
    assert!(is_absent_error(kAXErrorAttributeUnsupported));
    assert!(is_absent_error(kAXErrorNoValue));
    assert!(!is_absent_error(kAXErrorCannotComplete));
}

#[test]
fn bounded_child_read_distinguishes_truncation_from_native_failure() {
    let read = ChildRead {
        elements: Vec::new(),
        total_count: 4,
        complete: true,
        source_availability: ChildSourceAvailability::Available,
        prefix_certain: true,
        status: ChildReadStatus::default(),
    };

    assert!(read.truncated());
    assert!(read.complete);
}

#[test]
fn child_count_change_is_not_a_complete_observation() {
    assert!(count_changed(4, 3));
    assert!(count_changed(3, 4));
    assert!(!count_changed(4, 4));
}

#[test]
fn cursor_stall_is_preserved_when_statuses_merge() {
    let mut status = ChildReadStatus::default();
    status.merge(ChildReadStatus {
        cursor_stalled: true,
        health: agent_desktop_core::LocatorReadHealth {
            native_read_failures: 2,
            ..Default::default()
        },
        ..ChildReadStatus::default()
    });

    assert!(status.cursor_stalled);
    assert_eq!(status.health.native_read_failures, 2);
}

#[test]
fn indexed_child_read_is_not_rejected_for_unread_siblings() {
    let read = ChildRead {
        elements: vec![AXElement(std::ptr::null_mut())],
        total_count: 10_000,
        complete: true,
        source_availability: ChildSourceAvailability::Available,
        prefix_certain: true,
        status: ChildReadStatus::default(),
    };

    assert!(read.complete);
    assert!(read.truncated());
}

#[test]
fn positive_short_pages_continue_from_the_advanced_offset() {
    let mut calls = Vec::new();
    let result = read_paged_prefix(4, 128, |offset, maximum| {
        calls.push((offset, maximum));
        Ok(if offset == 0 { vec![0, 1] } else { vec![2, 3] })
    })
    .unwrap();

    assert_eq!(calls, [(0, 4), (2, 2)]);
    assert_eq!(result.elements, [0, 1, 2, 3]);
    assert!(!result.stalled);
}

#[test]
fn zero_progress_stalls_and_terminates_before_the_requested_count() {
    let mut calls = Vec::new();
    let result = read_paged_prefix(4, 128, |offset, maximum| {
        calls.push((offset, maximum));
        Ok(if offset == 0 { vec![0, 1] } else { Vec::new() })
    })
    .unwrap();

    assert_eq!(calls, [(0, 4), (2, 2)]);
    assert_eq!(result.elements, [0, 1]);
    assert!(result.stalled);
}

#[test]
fn multi_page_reads_preserve_order_beyond_the_native_page_size() {
    let mut calls = Vec::new();
    let result = read_paged_prefix(260, 128, |offset, maximum| {
        calls.push((offset, maximum));
        Ok((offset..offset + maximum).collect::<Vec<_>>())
    })
    .unwrap();

    assert_eq!(calls, [(0, 128), (128, 128), (256, 4)]);
    assert_eq!(result.elements, (0..260).collect::<Vec<_>>());
    assert!(!result.stalled);
}

#[test]
fn page_error_after_progress_is_not_collapsed_to_a_short_complete_read() {
    let error = read_paged_prefix(4, 128, |offset, _| {
        if offset == 0 {
            Ok(vec![0, 1])
        } else {
            Err(kAXErrorCannotComplete)
        }
    })
    .err()
    .expect("a later native error must remain visible");

    assert_eq!(error, kAXErrorCannotComplete);
}

#[test]
fn unclassified_native_error_is_counted_explicitly() {
    let mut status = ChildReadStatus::default();

    record_error(&mut status, i32::MIN);

    assert_eq!(status.health.native_read_failures, 1);
    assert_eq!(status.health.cannot_complete, 0);
    assert!(!status.invalid_element);
    assert!(!status.api_disabled);
}

#[test]
fn ax_failure_remains_a_native_read_failure() {
    let mut status = ChildReadStatus::default();

    record_error(&mut status, kAXErrorFailure);

    assert_eq!(status.health.native_read_failures, 1);
    assert_eq!(status.health.cannot_complete, 0);
}
