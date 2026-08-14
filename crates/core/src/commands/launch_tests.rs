use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::launch_options::LaunchOptions;
use crate::{AdapterError, InteractionLease, ProcessId, WindowInfo, WindowState};
use std::sync::atomic::{AtomicU64, Ordering};

struct LaunchAdapter {
    lease_timeout_ms: AtomicU64,
}

impl ObservationOps for LaunchAdapter {}
impl ActionOps for LaunchAdapter {}
impl InputOps for LaunchAdapter {}

impl SystemOps for LaunchAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        self.lease_timeout_ms
            .store(deadline.timeout_ms(), Ordering::SeqCst);
        InteractionLease::guarded(deadline, ())
    }

    fn launch_app(
        &self,
        _id: &str,
        _options: &LaunchOptions,
        _lease: &InteractionLease,
    ) -> Result<crate::launch_result::LaunchResult, AdapterError> {
        Ok(crate::launch_result::LaunchResult {
            app: "Fixture".into(),
            pid: ProcessId::new(42),
            process_instance: Some("42:1".into()),
            window: Some(WindowInfo {
                id: "w-1".into(),
                title: "Fixture".into(),
                app: "Fixture".into(),
                pid: ProcessId::new(42),
                process_instance: Some("42:1".into()),
                bounds: None,
                state: WindowState::default(),
            }),
        })
    }
}

#[test]
fn launch_lease_uses_the_requested_timeout() {
    let adapter = LaunchAdapter {
        lease_timeout_ms: AtomicU64::new(0),
    };
    let options = LaunchOptions {
        timeout_ms: 30_000,
        ..Default::default()
    };

    execute(
        LaunchArgs {
            app: "Fixture".into(),
            options,
        },
        &adapter,
    )
    .unwrap();

    assert_eq!(adapter.lease_timeout_ms.load(Ordering::SeqCst), 30_000);
}

#[test]
fn zero_launch_timeout_keeps_a_bounded_lease_for_the_single_attempt() {
    let adapter = LaunchAdapter {
        lease_timeout_ms: AtomicU64::new(0),
    };

    execute(
        LaunchArgs {
            app: "Fixture".into(),
            options: LaunchOptions {
                timeout_ms: 0,
                ..Default::default()
            },
        },
        &adapter,
    )
    .unwrap();

    assert_eq!(
        adapter.lease_timeout_ms.load(Ordering::SeqCst),
        crate::Deadline::standard().unwrap().timeout_ms()
    );
}
