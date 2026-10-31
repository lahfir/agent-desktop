use super::*;
use std::sync::Mutex;

struct RefWindowAdapter {
    events: Mutex<Vec<&'static str>>,
    mismatch: bool,
}

impl crate::ObservationOps for RefWindowAdapter {}
impl crate::ActionOps for RefWindowAdapter {}
impl crate::InputOps for RefWindowAdapter {}
impl crate::SystemOps for RefWindowAdapter {
    fn resolve_ref_focus_window(
        &self,
        entry: &crate::RefEntry,
        _: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.events.lock().unwrap().push("ref_window");
        assert!(entry.source.source_window_id.is_none());
        let mut evidence = entry.clone();
        evidence.source.source_window_id = Some("exact-live-owner".into());
        crate::saved_ref_focus_window(&evidence)
    }

    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.events.lock().unwrap().push("strict_window");
        assert_eq!(expected.id, "exact-live-owner");
        let mut live = expected.clone();
        if self.mismatch {
            live.process_instance = Some("different-generation".into());
        }
        Ok(live)
    }

    fn focus_window(
        &self,
        window: &WindowInfo,
        _: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        self.events.lock().unwrap().push("focus");
        assert_eq!(window.id, "exact-live-owner");
        Ok(())
    }
}

#[test]
fn recovered_ref_window_is_strictly_verified_before_focus_without_rewriting_source() {
    for mismatch in [false, true] {
        let adapter = RefWindowAdapter {
            events: Mutex::new(Vec::new()),
            mismatch,
        };
        let mut entry = crate::adapter::minimal_ref_entry("menuitem", None);
        entry.source.source_surface = crate::SnapshotSurface::Menu;
        let original = entry.clone();
        let lease =
            crate::InteractionLease::guarded(crate::Deadline::standard().unwrap(), ()).unwrap();
        let result =
            focus_entry_window(&entry, &adapter, &crate::CommandContext::default(), &lease);
        assert_eq!(entry, original);
        if mismatch {
            assert_eq!(result.unwrap_err().code(), "STALE_REF");
            assert_eq!(
                *adapter.events.lock().unwrap(),
                ["ref_window", "strict_window"]
            );
        } else {
            assert_eq!(result.unwrap().id, "exact-live-owner");
            assert_eq!(
                *adapter.events.lock().unwrap(),
                ["ref_window", "strict_window", "focus"]
            );
        }
    }
}
