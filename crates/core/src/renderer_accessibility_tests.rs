use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

struct LeaseFlag(Arc<AtomicBool>);

impl Drop for LeaseFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

struct RendererAdapter {
    lease_held: Arc<AtomicBool>,
    observations: AtomicU32,
    activations: AtomicU32,
}

impl ObservationOps for RendererAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, crate::AdapterError> {
        let attempt = self.observations.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            return Err(
                crate::AdapterError::renderer_accessibility_activation_required(
                    "activation required",
                ),
            );
        }
        assert!(!self.lease_held.load(Ordering::SeqCst));
        crate::adapter::observed_tree(
            &root,
            crate::AccessibilityNode {
                ref_id: None,
                role: "window".into(),
                identity: Default::default(),
                presentation: Default::default(),
                children_count: None,
                children: Vec::new(),
            },
        )
    }
}

impl ActionOps for RendererAdapter {}
impl InputOps for RendererAdapter {}

impl SystemOps for RendererAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, crate::AdapterError> {
        self.lease_held.store(true, Ordering::SeqCst);
        crate::InteractionLease::guarded(deadline, LeaseFlag(Arc::clone(&self.lease_held)))
    }

    fn activate_renderer_accessibility(
        &self,
        _process: crate::ProcessIdentity,
        _lease: &crate::InteractionLease,
    ) -> Result<(), crate::AdapterError> {
        assert!(self.lease_held.load(Ordering::SeqCst));
        self.activations.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn activation_lease_is_dropped_before_observation_retry() {
    let held = Arc::new(AtomicBool::new(false));
    let adapter = RendererAdapter {
        lease_held: Arc::clone(&held),
        observations: AtomicU32::new(0),
        activations: AtomicU32::new(0),
    };
    let window = crate::WindowInfo {
        id: "w-1".into(),
        title: "Fixture".into(),
        app: "Fixture".into(),
        pid: crate::ProcessId::new(42),
        process_instance: Some("instance-1".into()),
        bounds: None,
        state: Default::default(),
    };
    let deadline = crate::Deadline::after(500).unwrap();
    let request = ObservationRequest::snapshot(&crate::TreeOptions::default(), deadline)
        .validate()
        .unwrap();

    let tree = observe_tree(&adapter, ObservationRoot::Window(&window), &request).unwrap();

    assert_eq!(tree.node_count(), 1);
    assert_eq!(adapter.observations.load(Ordering::SeqCst), 2);
    assert_eq!(adapter.activations.load(Ordering::SeqCst), 1);
    assert!(!held.load(Ordering::SeqCst));
}
