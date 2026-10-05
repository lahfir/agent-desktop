use agent_desktop_core::{AppInventory, Deadline, ErrorCode, ObservationOps};
use std::cell::RefCell;

thread_local! {
    static WORKSPACE_INVENTORY: RefCell<Option<AppInventory>> = const { RefCell::new(None) };
}

pub(crate) fn workspace_inventory() -> Option<AppInventory> {
    WORKSPACE_INVENTORY.with_borrow(Clone::clone)
}

struct InventoryFixture;

impl Drop for InventoryFixture {
    fn drop(&mut self) {
        WORKSPACE_INVENTORY.set(None);
    }
}

#[test]
fn adapter_rejects_silently_partial_apps_but_inventory_reports_skipped() {
    let skipped = serde_json::json!({"pid":11,"field":"application_name"});
    WORKSPACE_INVENTORY.set(Some(AppInventory {
        apps: Vec::new(),
        skipped: vec![skipped.clone()],
    }));
    let _fixture = InventoryFixture;
    let adapter = crate::MacOSAdapter::new();
    let inventory = adapter
        .list_apps_inventory(Deadline::after(1000).unwrap())
        .unwrap();
    assert!(inventory.apps.is_empty());
    assert_eq!(inventory.skipped, vec![skipped.clone()]);

    let error = adapter
        .list_apps(Deadline::after(30).unwrap())
        .expect_err("a skipped record must not become a successful complete app list");
    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.unwrap();
    assert!(details["attempts"].as_u64().unwrap() > 1);
    assert_eq!(details["last_failure_details"]["skipped"][0], skipped);
}
