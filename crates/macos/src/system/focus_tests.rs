use super::*;
use std::cell::Cell;

#[test]
fn already_frontmost_does_not_activate() {
    assert!(
        !focus_until(
            Deadline::standard().unwrap(),
            || Ok(true),
            || panic!("already focused")
        )
        .unwrap()
    );
}

#[test]
fn activation_requires_frontmost_readback() {
    let activated = Cell::new(false);
    let reads = Cell::new(0);
    let delivered = focus_until(
        Deadline::standard().unwrap(),
        || {
            reads.set(reads.get() + 1);
            Ok(activated.get())
        },
        || {
            activated.set(true);
            Ok(true)
        },
    )
    .unwrap();
    assert!(delivered);
    assert_eq!(reads.get(), 2);
}

#[test]
fn identity_and_permission_errors_before_activation_prevent_delivery() {
    for code in [
        ErrorCode::StaleRef,
        ErrorCode::PermDenied,
        ErrorCode::Timeout,
    ] {
        let error = focus_until(
            Deadline::standard().unwrap(),
            || Err(AdapterError::new(code.clone(), "read failed")),
            || panic!("must not activate"),
        )
        .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    }
}

#[test]
fn failed_verification_preserves_delivery_and_original_error() {
    for delivered in [false, true] {
        let reads = Cell::new(0);
        let error = focus_until(
            Deadline::standard().unwrap(),
            || {
                reads.set(reads.get() + 1);
                if reads.get() == 1 {
                    Ok(false)
                } else {
                    Err(AdapterError::stale_ref_because("process restarted"))
                }
            },
            || Ok(delivered),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::StaleRef);
        assert_eq!(
            error.disposition,
            if delivered {
                DeliverySemantics::delivered_unverified()
            } else {
                AdapterError::stale_ref_because("process restarted").disposition
            }
        );
    }
}

#[test]
fn frontmost_verification_timeout_preserves_activation_delivery() {
    for delivered in [false, true] {
        let deadline = Deadline::after(100).unwrap();
        let reads = Cell::new(0);
        let error = focus_until(
            deadline,
            || {
                reads.set(reads.get() + 1);
                Ok(false)
            },
            || {
                std::thread::sleep(deadline.remaining());
                Ok(delivered)
            },
        )
        .unwrap_err();
        assert_eq!(reads.get(), 2);
        assert_eq!(error.code, ErrorCode::Timeout);
        assert_eq!(
            error.details.as_ref().unwrap()["kind"],
            "focus_verification"
        );
        assert_eq!(
            error.disposition,
            if delivered {
                DeliverySemantics::delivered_unverified()
            } else {
                AdapterError::timeout("").disposition
            }
        );
    }
}
