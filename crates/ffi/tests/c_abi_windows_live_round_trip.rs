#![cfg(target_os = "windows")]

mod common;

use agent_desktop_ffi::AdActionKind;
use common::win32_fixture::stage_click_fixture;
use common::{
    AdAction, AdDragParams, AdKeyCombo, AdPoint, AdPolicyKind, AdResult, AdScrollParams,
    ad_execute_by_ref, ad_free_string, ad_snapshot, with_adapter,
};
use std::ffi::{CStr, CString, c_char};

const SNAPSHOT_SURFACE_WINDOW: i32 = 0;
const POLICY_HEADLESS: i32 = AdPolicyKind::Headless as i32;
const CLICK_BUDGET_MS: u64 = 10_000;

fn click_action() -> AdAction {
    AdAction {
        kind: AdActionKind::Click as i32,
        text: std::ptr::null(),
        scroll: AdScrollParams {
            direction: 0,
            amount: 0,
        },
        key: AdKeyCombo {
            key: std::ptr::null(),
            modifiers: std::ptr::null(),
            modifier_count: 0,
        },
        drag: AdDragParams {
            from: AdPoint { x: 0.0, y: 0.0 },
            to: AdPoint { x: 0.0, y: 0.0 },
            duration_ms: 0,
            drop_delay_ms: 0,
        },
    }
}

unsafe fn envelope_json(pointer: *mut c_char) -> serde_json::Value {
    assert!(
        !pointer.is_null(),
        "command returned a null envelope pointer"
    );
    let json = unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned();
    unsafe { ad_free_string(pointer) };
    serde_json::from_str(&json).expect("envelope is valid JSON")
}

fn find_button_ref(node: &serde_json::Value, searched: &mut usize) -> Option<String> {
    *searched += 1;
    if node["role"] == "button"
        && node["name"]
            .as_str()
            .is_some_and(|name| name.contains("ffi-fixture-button"))
    {
        return node["ref_id"].as_str().map(str::to_string);
    }
    for child in node["children"].as_array()? {
        if let Some(found) = find_button_ref(child, searched) {
            return Some(found);
        }
    }
    None
}

#[test]
fn c_abi_click_on_a_real_window_is_confirmed_by_the_window_itself() {
    let fixture = stage_click_fixture("round-trip");
    assert_eq!(
        fixture.click_count(),
        0,
        "a freshly staged fixture must not carry clicks"
    );

    with_adapter(|adapter| unsafe {
        let app = CString::new(fixture.app_filter()).expect("app filter is NUL-free");
        let mut out: *mut c_char = std::ptr::null_mut();
        let status = ad_snapshot(
            adapter,
            app.as_ptr(),
            SNAPSHOT_SURFACE_WINDOW,
            12,
            false,
            false,
            &mut out,
        );
        assert_eq!(status, AdResult::Ok, "ad_snapshot failed");
        let envelope = envelope_json(out);
        assert_eq!(
            envelope["ok"].as_bool(),
            Some(true),
            "snapshot envelope reported failure: {envelope}"
        );

        let tree = envelope
            .pointer("/data/tree")
            .unwrap_or_else(|| panic!("snapshot envelope carried no data.tree: {envelope}"));
        let mut searched = 0usize;
        let ref_id = find_button_ref(tree, &mut searched).unwrap_or_else(|| {
            panic!(
                "no button named ffi-fixture-button in a tree of {searched} nodes; \
                     the fixture window was not snapshotted"
            )
        });
        assert!(
            ref_id.starts_with('@'),
            "expected a snapshot-qualified ref, got {ref_id}"
        );

        let ref_c = CString::new(ref_id.clone()).expect("ref id is NUL-free");
        let mut action_out: *mut c_char = std::ptr::null_mut();
        let status = ad_execute_by_ref(
            adapter,
            ref_c.as_ptr(),
            std::ptr::null(),
            &click_action(),
            POLICY_HEADLESS,
            &mut action_out,
        );
        assert_eq!(status, AdResult::Ok, "ad_execute_by_ref failed");
        let action_envelope = envelope_json(action_out);
        assert_eq!(
            action_envelope["ok"].as_bool(),
            Some(true),
            "execute_by_ref envelope reported failure: {action_envelope}"
        );

        fixture
            .wait_for_clicks(1, CLICK_BUDGET_MS)
            .unwrap_or_else(|error| {
                panic!("{error} — the click through the C ABI never reached the fixture window")
            });
    });

    assert_eq!(
        fixture.click_count(),
        1,
        "the fixture window must record exactly one click after one C ABI action"
    );
}
