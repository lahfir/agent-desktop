#![allow(dead_code, unused_imports)]
#![allow(improper_ctypes)]

pub use agent_desktop_ffi::error::AdResult;
pub use agent_desktop_ffi::{
    AdAction, AdActionResult, AdActionStep, AdAdapter, AdAppList, AdDirection, AdDragParams,
    AdElementState, AdFindQuery, AdKeyCombo, AdNativeHandle, AdPoint, AdPolicyKind, AdRect,
    AdRefEntry, AdScrollParams, AdWindowInfo, AdWindowList,
};
pub use std::ffi::CStr;
pub use std::os::raw::c_char;

unsafe extern "C" {
    pub fn ad_abi_version() -> u32;
    pub fn ad_init(expected_major: u32) -> AdResult;

    pub fn ad_ref_entry_size() -> usize;
    pub fn ad_action_size() -> usize;
    pub fn ad_action_step_size() -> usize;
    pub fn ad_action_result_size() -> usize;
    pub fn ad_element_state_size() -> usize;

    pub fn ad_adapter_create() -> *mut AdAdapter;
    pub fn ad_adapter_destroy(adapter: *mut AdAdapter);
    pub fn ad_check_permissions(adapter: *const AdAdapter) -> AdResult;

    pub fn ad_last_error_code() -> AdResult;
    pub fn ad_last_error_message() -> *const c_char;
    pub fn ad_last_error_details() -> *const c_char;

    pub fn ad_list_apps(adapter: *const AdAdapter, out: *mut *mut AdAppList) -> AdResult;
    pub fn ad_app_list_count(list: *const AdAppList) -> u32;
    pub fn ad_app_list_get(list: *const AdAppList, index: u32) -> *const u8;
    pub fn ad_app_list_free(list: *mut AdAppList);

    pub fn ad_list_windows(
        adapter: *const AdAdapter,
        app_filter: *const c_char,
        focused_only: bool,
        out: *mut *mut AdWindowList,
    ) -> AdResult;
    pub fn ad_window_list_count(list: *const AdWindowList) -> u32;
    pub fn ad_window_list_free(list: *mut AdWindowList);

    pub fn ad_launch_app(
        adapter: *const AdAdapter,
        id: *const c_char,
        timeout_ms: u64,
        out: *mut AdWindowInfo,
    ) -> AdResult;

    pub fn ad_execute_action(
        adapter: *const AdAdapter,
        handle: *const AdNativeHandle,
        action: *const AdAction,
        out: *mut AdActionResult,
    ) -> AdResult;
    pub fn ad_execute_action_with_policy(
        adapter: *const AdAdapter,
        handle: *const AdNativeHandle,
        action: *const AdAction,
        policy: i32,
        out: *mut AdActionResult,
    ) -> AdResult;
    pub fn ad_execute_ref_action_with_policy(
        adapter: *const AdAdapter,
        entry: *const AdRefEntry,
        action: *const AdAction,
        policy: i32,
        out: *mut AdActionResult,
    ) -> AdResult;
    pub fn ad_free_action_result(result: *mut AdActionResult);

    pub fn ad_find(
        adapter: *const AdAdapter,
        win: *const AdWindowInfo,
        query: *const AdFindQuery,
        out: *mut AdNativeHandle,
    ) -> AdResult;

    pub fn ad_free_handle(adapter: *const AdAdapter, handle: *mut AdNativeHandle) -> AdResult;

    pub fn ad_resolve_element(
        adapter: *const AdAdapter,
        entry: *const AdRefEntry,
        out: *mut AdNativeHandle,
    ) -> AdResult;
}

pub fn with_adapter<F: FnOnce(*mut AdAdapter)>(body: F) {
    unsafe {
        let adapter = ad_adapter_create();
        assert!(!adapter.is_null(), "ad_adapter_create must not return null");
        body(adapter);
        ad_adapter_destroy(adapter);
    }
}

pub fn default_ref_entry() -> AdRefEntry {
    AdRefEntry {
        pid: 0,
        role: std::ptr::null(),
        name: std::ptr::null(),
        value: std::ptr::null(),
        description: std::ptr::null(),
        states: std::ptr::null(),
        state_count: 0,
        available_actions: std::ptr::null(),
        available_action_count: 0,
        bounds: AdRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        has_bounds: false,
        bounds_hash: 0,
        has_bounds_hash: false,
        source_app: std::ptr::null(),
        source_window_id: std::ptr::null(),
        source_window_title: std::ptr::null(),
        source_surface: 0,
        root_ref: std::ptr::null(),
        path_is_absolute: false,
        path: std::ptr::null(),
        path_count: 0,
    }
}

pub fn default_action() -> AdAction {
    AdAction {
        kind: 0,
        text: std::ptr::null(),
        scroll: AdScrollParams {
            direction: AdDirection::Down as i32,
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
