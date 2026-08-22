use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};

struct WindowAdapter {
    windows: Vec<WindowInfo>,
}

impl ObservationOps for WindowAdapter {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, crate::AdapterError> {
        Ok(self.windows.clone())
    }
}

impl ActionOps for WindowAdapter {}
impl InputOps for WindowAdapter {}
impl SystemOps for WindowAdapter {}

fn window(app: &str) -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "WhatsApp".into(),
        app: app.into(),
        pid: crate::ProcessId::new(10),
        process_instance: Some("instance-10".into()),
        bounds: None,
        state: Default::default(),
    }
}

#[test]
fn adapter_owned_app_resolution_preserves_bundle_identifier_matches() {
    let adapter = WindowAdapter {
        windows: vec![window("WhatsApp")],
    };

    let resolved = resolve_window(
        &adapter,
        Some("net.whatsapp.WhatsApp"),
        None,
        crate::Deadline::after(100).unwrap(),
    )
    .unwrap();

    assert_eq!(resolved.id, "w-1");
}

#[test]
fn running_application_without_a_window_is_not_reported_as_absent() {
    let adapter = WindowAdapter { windows: vec![] };

    let error = resolve_window(
        &adapter,
        Some("WhatsApp"),
        None,
        crate::Deadline::after(100).unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.code(), "WINDOW_NOT_FOUND");
}
