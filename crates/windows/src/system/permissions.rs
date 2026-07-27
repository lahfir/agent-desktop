use agent_desktop_core::{AdapterError, Deadline, PermissionReport, PermissionState};

const ACCESSIBILITY_SUGGESTION: &str = "Run agent-desktop in an interactive desktop session as a user allowed to use the UI Automation COM runtime; restricted tokens and AppContainer processes are denied UIA access.";

const S_OK: i32 = 0;
const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;

#[cfg(target_os = "windows")]
mod imp {
    use windows_sys::Win32::Foundation::{S_FALSE, S_OK};
    use windows_sys::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    };
    use windows_sys::core::GUID;

    const CLSID_CUIAUTOMATION: GUID = GUID::from_u128(0xff48dba4_60ef_4201_aa87_54103eef594e);
    const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
    const RPC_E_CHANGED_MODE: i32 = 0x8001_0106_u32 as i32;

    #[repr(C)]
    struct ComObject {
        vtable: *const ComVtable,
    }

    #[repr(C)]
    struct ComVtable {
        query_interface: usize,
        add_ref: usize,
        release: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
    }

    const _: () = assert!(size_of::<ComVtable>() == 3 * size_of::<usize>());

    pub(super) fn probe_uia_access() -> i32 {
        unsafe {
            let init_status = CoInitializeEx(core::ptr::null(), COINIT_MULTITHREADED as u32);
            if init_status < 0 && init_status != RPC_E_CHANGED_MODE {
                return init_status;
            }
            let balance_apartment = init_status == S_OK || init_status == S_FALSE;
            let mut instance: *mut core::ffi::c_void = core::ptr::null_mut();
            let create_status = CoCreateInstance(
                &CLSID_CUIAUTOMATION,
                core::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_IUNKNOWN,
                &mut instance,
            );
            release_instance(instance);
            if balance_apartment {
                CoUninitialize();
            }
            create_status
        }
    }

    unsafe fn release_instance(instance: *mut core::ffi::c_void) {
        if instance.is_null() {
            return;
        }
        let object = instance.cast::<ComObject>();
        unsafe {
            ((*(*object).vtable).release)(instance);
        }
    }

    pub(super) fn probe_capture_availability() -> Option<bool> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    const E_NOTIMPL: i32 = 0x8000_4001_u32 as i32;

    pub(super) fn probe_uia_access() -> i32 {
        E_NOTIMPL
    }

    pub(super) fn probe_capture_availability() -> Option<bool> {
        None
    }
}

pub(crate) fn report(deadline: Deadline) -> Result<PermissionReport, AdapterError> {
    ensure_budget(deadline)?;
    let report = PermissionReport {
        accessibility: accessibility_report_state(),
        screen_recording: screen_recording_report_state(),
        automation: automation_report_state(),
    };
    ensure_budget(deadline)?;
    Ok(report)
}

pub(crate) fn request_report(deadline: Deadline) -> Result<PermissionReport, AdapterError> {
    request_report_with(deadline, imp::probe_uia_access, report)
}

fn request_report_with(
    deadline: Deadline,
    probe: impl FnOnce() -> i32,
    report: impl FnOnce(Deadline) -> Result<PermissionReport, AdapterError>,
) -> Result<PermissionReport, AdapterError> {
    ensure_budget(deadline)?;
    let hresult = probe();
    ensure_budget(deadline)?;
    if matches!(map_uia_access(hresult), PermissionState::Denied { .. }) {
        return Err(uia_access_denied_error(hresult));
    }
    report(deadline)
}

pub(crate) fn map_uia_access(hresult: i32) -> PermissionState {
    match hresult {
        S_OK => PermissionState::Granted,
        E_ACCESSDENIED => PermissionState::Denied {
            suggestion: ACCESSIBILITY_SUGGESTION.into(),
        },
        _ => PermissionState::Unknown,
    }
}

pub(crate) fn map_capture_availability(availability: Option<bool>) -> PermissionState {
    match availability {
        Some(true) => PermissionState::NotRequired,
        Some(false) | None => PermissionState::Unknown,
    }
}

pub(crate) fn uia_access_denied_error(hresult: i32) -> AdapterError {
    AdapterError::new(
        agent_desktop_core::ErrorCode::PermDenied,
        "UI Automation access is denied for this process",
    )
    .with_suggestion(ACCESSIBILITY_SUGGESTION)
    .with_platform_detail(com_hresult_detail(hresult))
}

pub(crate) fn com_hresult_detail(hresult: i32) -> String {
    let code = hresult as u32;
    match hresult {
        E_ACCESSDENIED => format!("COM HRESULT 0x{code:08X} (E_ACCESSDENIED: Access is denied)"),
        _ => format!("COM HRESULT 0x{code:08X}"),
    }
}

fn accessibility_report_state() -> PermissionState {
    map_uia_access(imp::probe_uia_access())
}

fn screen_recording_report_state() -> PermissionState {
    map_capture_availability(imp::probe_capture_availability())
}

fn automation_report_state() -> PermissionState {
    PermissionState::NotRequired
}

fn ensure_budget(deadline: Deadline) -> Result<(), AdapterError> {
    if deadline.is_expired() {
        Err(deadline.timeout_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
