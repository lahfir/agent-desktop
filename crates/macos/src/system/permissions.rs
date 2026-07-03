use agent_desktop_core::{PermissionReport, PermissionState};

const ACCESSIBILITY_SUGGESTION: &str = "Open System Settings > Privacy & Security > Accessibility and add the app that launches agent-desktop, such as Terminal, iTerm, or Codex. If macOS lists the built binary separately, add that binary too.";
const SCREEN_RECORDING_SUGGESTION: &str = "Open System Settings > Privacy & Security > Screen Recording and add the app that launches agent-desktop, such as Terminal, iTerm, or Codex. If macOS lists the built binary separately, add that binary too.";
pub(crate) const AUTOMATION_SUGGESTION: &str = "Open System Settings > Privacy & Security > Automation and allow the app that launches agent-desktop, such as Terminal, iTerm, or Codex, to control System Events. If macOS lists the built binary separately, add that binary too.";

#[cfg(target_os = "macos")]
mod imp {
    use accessibility_sys::{
        AXIsProcessTrusted, AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt,
    };
    use core_foundation::{
        base::TCFType, boolean::CFBoolean, dictionary::CFDictionary, string::CFString,
    };

    pub(super) fn is_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    pub(super) fn request_trust() -> bool {
        unsafe {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let val = CFBoolean::true_value();
            let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), val.as_CFType())]);
            AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef())
        }
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    pub(super) fn screen_recording_granted() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    pub(super) fn request_screen_recording() -> bool {
        unsafe { CGRequestScreenCaptureAccess() }
    }

    const TYPE_APPLICATION_BUNDLE_ID: u32 = 0x6275_6E64;
    const SYSTEM_EVENTS_BUNDLE_ID: &[u8] = b"com.apple.systemevents";

    #[repr(C)]
    struct AEAddressDesc {
        descriptor_type: u32,
        data_handle: *mut std::ffi::c_void,
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AECreateDesc(
            type_code: u32,
            data_ptr: *const std::ffi::c_void,
            data_size: isize,
            result: *mut AEAddressDesc,
        ) -> i32;
        fn AEDisposeDesc(the_aedesc: *mut AEAddressDesc) -> i32;
        fn AEDeterminePermissionToAutomateTarget(
            target: *const AEAddressDesc,
            ask_user_if_needed: bool,
            automated_allowed: *mut bool,
        ) -> i32;
    }

    pub(super) struct AutomationProbeOutcome {
        pub status: i32,
        pub allowed: bool,
    }

    pub(super) fn probe_automation_permission() -> AutomationProbeOutcome {
        unsafe {
            let mut target = AEAddressDesc {
                descriptor_type: 0,
                data_handle: std::ptr::null_mut(),
            };
            let create_status = AECreateDesc(
                TYPE_APPLICATION_BUNDLE_ID,
                SYSTEM_EVENTS_BUNDLE_ID.as_ptr().cast(),
                SYSTEM_EVENTS_BUNDLE_ID.len() as isize,
                &mut target,
            );
            if create_status != 0 {
                return AutomationProbeOutcome {
                    status: create_status,
                    allowed: false,
                };
            }

            let mut allowed = false;
            let status = AEDeterminePermissionToAutomateTarget(&target, false, &mut allowed);
            let _ = AEDisposeDesc(&mut target);
            AutomationProbeOutcome { status, allowed }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn is_trusted() -> bool {
        false
    }
    pub fn request_trust() -> bool {
        false
    }
    pub fn screen_recording_granted() -> bool {
        false
    }
    pub fn request_screen_recording() -> bool {
        false
    }
    pub struct AutomationProbeOutcome {
        pub status: i32,
        pub allowed: bool,
    }
    pub fn probe_automation_permission() -> AutomationProbeOutcome {
        AutomationProbeOutcome {
            status: -600,
            allowed: false,
        }
    }
}

pub fn report() -> PermissionReport {
    PermissionReport {
        accessibility: accessibility_report_state(),
        screen_recording: screen_recording_report_state(),
        automation: automation_report_state(),
    }
}

pub fn request_report() -> PermissionReport {
    PermissionReport {
        accessibility: permission_state(imp::request_trust(), ACCESSIBILITY_SUGGESTION),
        screen_recording: permission_state(
            imp::request_screen_recording(),
            SCREEN_RECORDING_SUGGESTION,
        ),
        automation: automation_report_state(),
    }
}

fn permission_state(granted: bool, suggestion: &'static str) -> PermissionState {
    if granted {
        PermissionState::Granted
    } else {
        PermissionState::Denied {
            suggestion: suggestion.into(),
        }
    }
}

fn accessibility_report_state() -> PermissionState {
    permission_state(imp::is_trusted(), ACCESSIBILITY_SUGGESTION)
}

fn screen_recording_report_state() -> PermissionState {
    permission_state(imp::screen_recording_granted(), SCREEN_RECORDING_SUGGESTION)
}

fn automation_report_state() -> PermissionState {
    let outcome = imp::probe_automation_permission();
    map_automation_probe(outcome.status, outcome.allowed)
}

pub(crate) fn map_automation_probe(status: i32, allowed: bool) -> PermissionState {
    const NO_ERR: i32 = 0;
    const PROC_NOT_FOUND: i32 = -600;
    const ERR_AE_EVENT_NOT_PERMITTED: i32 = -1743;
    const ERR_AE_EVENT_WOULD_REQUIRE_USER_CONSENT: i32 = -1744;

    match status {
        PROC_NOT_FOUND | ERR_AE_EVENT_WOULD_REQUIRE_USER_CONSENT => PermissionState::Unknown,
        ERR_AE_EVENT_NOT_PERMITTED => PermissionState::Denied {
            suggestion: AUTOMATION_SUGGESTION.into(),
        },
        NO_ERR if allowed => PermissionState::Granted,
        NO_ERR => PermissionState::Denied {
            suggestion: AUTOMATION_SUGGESTION.into(),
        },
        _ => PermissionState::Unknown,
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
