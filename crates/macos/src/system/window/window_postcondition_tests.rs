use super::*;

#[test]
fn geometry_verification_tolerates_ax_rounding_only() {
    let expected = Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 80.0,
    };
    let rounded = Rect {
        x: 11.0,
        y: 19.0,
        width: 99.0,
        height: 81.0,
    };
    let wrong = Rect {
        width: 90.0,
        ..rounded
    };

    assert!(geometry_matches(rounded, expected, true));
    assert!(geometry_matches(rounded, expected, false));
    assert!(!geometry_matches(wrong, expected, false));
}

fn surface_read_error(ax_error: i32) -> AdapterError {
    AdapterError::timeout("Accessibility surface read failed for AXMinimized").with_details(
        serde_json::json!({"attribute": "AXMinimized", "ax_error": ax_error, "kind": "surface_read"}),
    )
}

#[test]
fn busy_read_during_dock_animation_keeps_polling() {
    let mut reads = 0;
    let result = wait_until(Deadline::detached_after(2_000).expect("deadline"), || {
        reads += 1;
        if reads < 3 {
            Err(surface_read_error(CANNOT_COMPLETE))
        } else {
            Ok(true)
        }
    });

    assert!(result.is_ok());
    assert_eq!(reads, 3);
}

#[test]
fn non_transient_read_error_still_fails_at_once() {
    let mut reads = 0;
    let error = wait_until(Deadline::detached_after(2_000).expect("deadline"), || {
        reads += 1;
        Err(surface_read_error(-25202))
    })
    .expect_err("an invalid element is not transient");

    assert_eq!(reads, 1);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn busy_reads_until_the_deadline_report_the_last_read_error() {
    let error = wait_until(Deadline::detached_after(50).expect("deadline"), || {
        Err(surface_read_error(CANNOT_COMPLETE))
    })
    .expect_err("a read that never settles must fail");

    assert!(is_transient_read(&error));
}

#[test]
fn a_plain_not_yet_read_clears_an_earlier_busy_read() {
    let mut reads = 0;
    let error = wait_until(Deadline::detached_after(50).expect("deadline"), || {
        reads += 1;
        if reads == 1 {
            Err(surface_read_error(CANNOT_COMPLETE))
        } else {
            Ok(false)
        }
    })
    .expect_err("a window that never settles must fail");

    assert!(!is_transient_read(&error));
    assert!(error.message.contains("requested postcondition"));
}
