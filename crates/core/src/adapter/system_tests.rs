use super::*;
use crate::error::ErrorCode;
use crate::session_affinity::SessionAffinity;

struct DefaultOnly;
impl SystemOps for DefaultOnly {}

#[test]
fn default_open_session_is_not_supported() {
    let result = DefaultOnly.open_session(&SessionAffinity::default());
    let Err(err) = result else {
        panic!("expected open_session default to return an error");
    };

    assert_eq!(err.code, ErrorCode::PlatformNotSupported);
    assert!(
        err.message.contains("open_session"),
        "not_supported message should name the method, got: {}",
        err.message
    );
}
