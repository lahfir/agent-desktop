use crate::AdAdapter;
use crate::convert::surface::{
    exact_surface_info_to_c, free_exact_surface_info_fields, free_surface_info_fields,
    surface_info_to_c, validate_surface_info,
};
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::{trap_panic, trap_panic_void};
use crate::types::{AdExactSurfaceInfo, AdExactSurfaceList, AdSurfaceInfo, AdSurfaceList};
use std::ptr;

/// # Safety
/// `adapter` must be valid. `out` must be a valid writable
/// `*mut *mut AdSurfaceList`. Success produces a list handle freed via
/// `ad_surface_list_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_list_surfaces(
    adapter: *const AdAdapter,
    pid: u32,
    out: *mut *mut AdSurfaceList,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = crate::adapter::acquire_adapter!(adapter);
        let deadline = crate::operation::operation_deadline!();
        let process = match process_identity_for_pid(adapter.inner.as_ref(), pid, deadline) {
            Ok(process) => process,
            Err(error) => {
                set_last_error(&error);
                return crate::error::last_error_code();
            }
        };
        match adapter.inner.list_surfaces(process, deadline) {
            Ok(surfaces) => {
                if let Err(error) =
                    crate::resource::validate_list_len(surfaces.len(), "Surface list")
                {
                    set_last_error(&error);
                    return crate::error::last_error_code();
                }
                if let Err(error) = surfaces.iter().try_for_each(validate_surface_info) {
                    set_last_error(&error);
                    return crate::error::last_error_code();
                }
                let items: Vec<AdSurfaceInfo> = surfaces.iter().map(surface_info_to_c).collect();
                let list = Box::new(AdSurfaceList {
                    items: items.into_boxed_slice(),
                });
                *out = Box::into_raw(list);
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}

/// # Safety
/// `list` must be null or a pointer returned by `ad_list_surfaces`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_surface_list_count(list: *const AdSurfaceList) -> u32 {
    if list.is_null() {
        return 0;
    }
    let list_ref: &AdSurfaceList = unsafe { &*list };
    list_ref.items.len() as u32
}

/// Borrow a surface info entry. Null if `index` is out of range.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_surfaces`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_surface_list_get(
    list: *const AdSurfaceList,
    index: u32,
) -> *const AdSurfaceInfo {
    if list.is_null() {
        return ptr::null();
    }
    let list_ref: &AdSurfaceList = unsafe { &*list };
    match list_ref.items.get(index as usize) {
        Some(item) => item as *const AdSurfaceInfo,
        None => ptr::null(),
    }
}

/// Frees the list and each entry's interior strings.
///
/// # Safety
/// `list` must be null or a pointer returned by `ad_list_surfaces`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_surface_list_free(list: *mut AdSurfaceList) {
    trap_panic_void(|| unsafe {
        if list.is_null() {
            return;
        }
        let mut list = Box::from_raw(list);
        for item in list.items.iter_mut() {
            free_surface_info_fields(item);
        }
    })
}

/// Lists surfaces without dropping their core surface IDs.
///
/// # Safety
/// `adapter` and `out` must be valid. The returned list must be freed with
/// `ad_exact_surface_list_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_list_surfaces_exact(
    adapter: *const AdAdapter,
    pid: u32,
    out: *mut *mut AdExactSurfaceList,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = crate::adapter::acquire_adapter!(adapter);
        let deadline = crate::operation::operation_deadline!();
        let process = match process_identity_for_pid(adapter.inner.as_ref(), pid, deadline) {
            Ok(process) => process,
            Err(error) => {
                set_last_error(&error);
                return crate::error::last_error_code();
            }
        };
        match adapter.inner.list_surfaces(process, deadline) {
            Ok(surfaces) => {
                if let Err(error) =
                    crate::resource::validate_list_len(surfaces.len(), "Exact surface list")
                {
                    set_last_error(&error);
                    return crate::error::last_error_code();
                }
                if let Err(error) = surfaces.iter().try_for_each(validate_surface_info) {
                    set_last_error(&error);
                    return crate::error::last_error_code();
                }
                let items: Vec<AdExactSurfaceInfo> =
                    surfaces.iter().map(exact_surface_info_to_c).collect();
                *out = Box::into_raw(Box::new(AdExactSurfaceList {
                    items: items.into_boxed_slice(),
                }));
                AdResult::Ok
            }
            Err(error) => {
                set_last_error(&error);
                crate::error::last_error_code()
            }
        }
    })
}

fn process_identity_for_pid(
    adapter: &dyn agent_desktop_core::PlatformAdapter,
    pid: u32,
    deadline: agent_desktop_core::Deadline,
) -> Result<agent_desktop_core::ProcessIdentity, agent_desktop_core::AdapterError> {
    if pid == 0 {
        return Err(agent_desktop_core::AdapterError::new(
            agent_desktop_core::ErrorCode::InvalidArgs,
            "surface pid must be positive",
        ));
    }
    let mut matches = adapter
        .list_apps(deadline)?
        .into_iter()
        .filter(|app| app.pid.get() == pid)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(agent_desktop_core::AdapterError::new(
            agent_desktop_core::ErrorCode::AppNotFound,
            "surface pid did not identify a live application instance",
        ));
    }
    if matches.len() > 1 {
        return Err(agent_desktop_core::AdapterError::ambiguous_target(
            "surface pid identified multiple live application instances",
        ));
    }
    let app = matches.swap_remove(0);
    let instance = app
        .process_instance
        .filter(|instance| !instance.is_empty())
        .ok_or_else(|| {
            agent_desktop_core::AdapterError::new(
                agent_desktop_core::ErrorCode::ActionNotSupported,
                "surface application has no process-generation identity",
            )
        })?;
    Ok(agent_desktop_core::ProcessIdentity::new(
        agent_desktop_core::ProcessId::new(pid),
        instance,
    ))
}

/// # Safety
/// `list` must be null or returned by `ad_list_surfaces_exact`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_exact_surface_list_count(list: *const AdExactSurfaceList) -> u32 {
    if list.is_null() {
        return 0;
    }
    unsafe { &*list }.items.len() as u32
}

/// # Safety
/// `list` must be null or returned by `ad_list_surfaces_exact`. The result is
/// borrowed until the list is freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_exact_surface_list_get(
    list: *const AdExactSurfaceList,
    index: u32,
) -> *const AdExactSurfaceInfo {
    if list.is_null() {
        return ptr::null();
    }
    unsafe { &*list }
        .items
        .get(index as usize)
        .map_or(ptr::null(), std::ptr::from_ref)
}

/// # Safety
/// `list` must be null or returned by `ad_list_surfaces_exact`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_exact_surface_list_free(list: *mut AdExactSurfaceList) {
    trap_panic_void(|| unsafe {
        if list.is_null() {
            return;
        }
        let mut list = Box::from_raw(list);
        for item in &mut list.items {
            free_exact_surface_info_fields(item);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
    use agent_desktop_core::{AppInfo, SurfaceInfo};

    struct AppInventory(Vec<AppInfo>);

    impl ActionOps for AppInventory {}
    impl InputOps for AppInventory {}
    impl SystemOps for AppInventory {}

    impl ObservationOps for AppInventory {
        fn list_apps(
            &self,
            _deadline: agent_desktop_core::Deadline,
        ) -> Result<Vec<AppInfo>, agent_desktop_core::AdapterError> {
            Ok(self.0.clone())
        }
    }

    fn app(pid: u32, instance: &str) -> AppInfo {
        AppInfo {
            name: "Fixture".into(),
            pid: agent_desktop_core::ProcessId::new(pid),
            bundle_id: None,
            process_instance: Some(instance.into()),
        }
    }

    #[test]
    fn missing_surface_pid_is_app_not_found() {
        let error = process_identity_for_pid(
            &AppInventory(Vec::new()),
            42,
            agent_desktop_core::Deadline::standard().unwrap(),
        )
        .unwrap_err();

        assert_eq!(error.code, agent_desktop_core::ErrorCode::AppNotFound);
    }

    #[test]
    fn duplicate_surface_pid_is_ambiguous() {
        let error = process_identity_for_pid(
            &AppInventory(vec![app(42, "a"), app(42, "b")]),
            42,
            agent_desktop_core::Deadline::standard().unwrap(),
        )
        .unwrap_err();

        assert_eq!(error.code, agent_desktop_core::ErrorCode::AmbiguousTarget);
    }

    #[test]
    fn exact_surface_list_owns_borrowed_entries_until_explicit_free() {
        let item = exact_surface_info_to_c(&SurfaceInfo {
            id: "surface-1".into(),
            kind: "window".into(),
            title: None,
            item_count: Some(3),
        });
        let list = Box::into_raw(Box::new(AdExactSurfaceList {
            items: vec![item].into_boxed_slice(),
        }));

        assert_eq!(unsafe { ad_exact_surface_list_count(list) }, 1);
        assert!(!unsafe { ad_exact_surface_list_get(list, 0) }.is_null());
        assert!(unsafe { ad_exact_surface_list_get(list, 1) }.is_null());
        unsafe { ad_exact_surface_list_free(list) };
        unsafe { ad_exact_surface_list_free(ptr::null_mut()) };
    }
}
