use agent_desktop_core::{AdapterError, Deadline, PermissionReport, PermissionState};

const ACCESSIBILITY_SUGGESTION: &str = "Run agent-desktop in an interactive desktop session as a user allowed to use the UI Automation COM runtime; restricted tokens and AppContainer processes are denied UIA access.";

const S_OK: i32 = 0;
const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002_u32 as i32;
const E_POINTER: i32 = 0x8000_4003_u32 as i32;
const E_FAIL: i32 = 0x8000_4005_u32 as i32;
const E_INVALIDARG: i32 = 0x8007_0057_u32 as i32;
const CO_E_NOTINITIALIZED: i32 = 0x8004_01F0_u32 as i32;
const RPC_E_SERVERFAULT: i32 = 0x8001_0105_u32 as i32;
const RPC_E_DISCONNECTED: i32 = 0x8001_0108_u32 as i32;
const RPC_S_SERVER_UNAVAILABLE: i32 = 0x8007_06BA_u32 as i32;
const RPC_S_CALL_FAILED: i32 = 0x8007_06BE_u32 as i32;
const UIA_E_ELEMENTNOTENABLED: i32 = 0x8004_0200_u32 as i32;
const UIA_E_ELEMENTNOTAVAILABLE: i32 = 0x8004_0201_u32 as i32;
const UIA_E_NOCLICKABLEPOINT: i32 = 0x8004_0202_u32 as i32;
const UIA_E_PROXYASSEMBLYNOTLOADED: i32 = 0x8004_0203_u32 as i32;
const UIA_E_NOTSUPPORTED: i32 = 0x8004_0204_u32 as i32;
const UIA_E_TIMEOUT: i32 = 0x8013_1505_u32 as i32;
const UIA_E_INVALIDOPERATION: i32 = 0x8013_1509_u32 as i32;

#[cfg(target_os = "windows")]
mod imp {
    use windows_sys::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
        CoUninitialize,
    };
    use windows_sys::core::{GUID, IID_IUnknown, IUnknown_Vtbl};

    use crate::system::com_runtime::classify_co_initialize_hresult;

    const CLSID_CUIAUTOMATION: GUID = GUID::from_u128(0xff48dba4_60ef_4201_aa87_54103eef594e);

    pub(super) fn probe_uia_access() -> i32 {
        unsafe {
            let init_status = CoInitializeEx(core::ptr::null(), COINIT_MULTITHREADED as u32);
            let apartment = match classify_co_initialize_hresult(init_status) {
                Ok(apartment) => apartment,
                Err(failure) => return failure,
            };
            let mut instance: *mut core::ffi::c_void = core::ptr::null_mut();
            let create_status = CoCreateInstance(
                &CLSID_CUIAUTOMATION,
                core::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_IUnknown,
                &mut instance,
            );
            release_instance(instance);
            if apartment.permits_co_uninitialize() {
                CoUninitialize();
            }
            create_status
        }
    }

    unsafe fn release_instance(instance: *mut core::ffi::c_void) {
        if instance.is_null() {
            return;
        }
        let vtable = unsafe { *instance.cast::<*const IUnknown_Vtbl>() };
        unsafe {
            ((*vtable).Release)(instance);
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
    report_from_probed_uia(deadline, imp::probe_uia_access())
}

pub(crate) fn request_report(deadline: Deadline) -> Result<PermissionReport, AdapterError> {
    request_report_with(deadline, imp::probe_uia_access, report_from_probed_uia)
}

fn request_report_with(
    deadline: Deadline,
    probe: impl FnOnce() -> i32,
    report: impl FnOnce(Deadline, i32) -> Result<PermissionReport, AdapterError>,
) -> Result<PermissionReport, AdapterError> {
    ensure_budget(deadline)?;
    let hresult = probe();
    ensure_budget(deadline)?;
    if matches!(map_uia_access(hresult), PermissionState::Denied { .. }) {
        return Err(uia_access_denied_error(hresult));
    }
    report(deadline, hresult)
}

fn report_from_probed_uia(
    deadline: Deadline,
    uia_hresult: i32,
) -> Result<PermissionReport, AdapterError> {
    let report = PermissionReport {
        accessibility: map_uia_access(uia_hresult),
        screen_recording: screen_recording_report_state(),
        automation: automation_report_state(),
    };
    ensure_budget(deadline)?;
    Ok(report)
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
    match com_hresult_symbol(hresult) {
        Some((symbol, meaning)) => format!("COM HRESULT 0x{code:08X} ({symbol}: {meaning})"),
        None => format!("COM HRESULT 0x{code:08X}"),
    }
}

/// Names the HRESULTs the UI Automation client path can raise, so
/// `platform_detail` carries a symbol rather than a bare hexadecimal code.
///
/// The table is shape only: no entry derives from an observed application.
pub(crate) fn com_hresult_symbol(hresult: i32) -> Option<(&'static str, &'static str)> {
    let symbol = match hresult {
        E_ACCESSDENIED => ("E_ACCESSDENIED", "Access is denied"),
        E_NOINTERFACE => ("E_NOINTERFACE", "No such interface supported"),
        E_POINTER => ("E_POINTER", "Invalid pointer"),
        E_FAIL => ("E_FAIL", "Unspecified failure"),
        E_INVALIDARG => ("E_INVALIDARG", "One or more arguments are invalid"),
        CO_E_NOTINITIALIZED => ("CO_E_NOTINITIALIZED", "COM has not been initialized"),
        RPC_E_SERVERFAULT => ("RPC_E_SERVERFAULT", "The server raised an exception"),
        RPC_E_DISCONNECTED => ("RPC_E_DISCONNECTED", "The object invoked has disconnected"),
        RPC_S_SERVER_UNAVAILABLE => ("RPC_S_SERVER_UNAVAILABLE", "The RPC server is unavailable"),
        RPC_S_CALL_FAILED => ("RPC_S_CALL_FAILED", "The remote procedure call failed"),
        UIA_E_ELEMENTNOTENABLED => ("UIA_E_ELEMENTNOTENABLED", "The element is not enabled"),
        UIA_E_ELEMENTNOTAVAILABLE => ("UIA_E_ELEMENTNOTAVAILABLE", "The element is not available"),
        UIA_E_NOCLICKABLEPOINT => (
            "UIA_E_NOCLICKABLEPOINT",
            "The element has no clickable point",
        ),
        UIA_E_PROXYASSEMBLYNOTLOADED => (
            "UIA_E_PROXYASSEMBLYNOTLOADED",
            "The proxy assembly could not be loaded",
        ),
        UIA_E_NOTSUPPORTED => (
            "UIA_E_NOTSUPPORTED",
            "The requested operation is unsupported",
        ),
        UIA_E_TIMEOUT => ("UIA_E_TIMEOUT", "The operation timed out"),
        UIA_E_INVALIDOPERATION => ("UIA_E_INVALIDOPERATION", "The operation is not valid"),
        _ => return None,
    };
    Some(symbol)
}

fn screen_recording_report_state() -> PermissionState {
    map_capture_availability(imp::probe_capture_availability())
}

fn automation_report_state() -> PermissionState {
    PermissionState::NotRequired
}

pub(crate) fn ensure_budget(deadline: Deadline) -> Result<(), AdapterError> {
    if deadline.is_expired() {
        Err(deadline.timeout_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
