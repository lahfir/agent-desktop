use agent_desktop_core::{AdapterError, AppError, DeliveryDisposition, RetryDisposition};

#[path = "batch_seen_set_tests.rs"]
mod batch_seen_set_tests;

#[test]
fn pre_dispatch_failures_are_always_safe_to_retry() {
    for error in [
        AppError::from(AdapterError::internal("trace setup failed")),
        AppError::from(std::io::Error::other("read failed")),
    ] {
        let AppError::Adapter(error) = crate::pre_dispatch_error(error) else {
            panic!("pre-dispatch error must be normalized to AdapterError");
        };
        assert_eq!(error.disposition.retry(), RetryDisposition::Safe);
        assert_eq!(
            error.disposition.delivery(),
            DeliveryDisposition::NotDelivered
        );
    }
}
