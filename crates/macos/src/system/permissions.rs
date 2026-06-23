use agent_desktop_core::{PermissionReport, PermissionState};

const ACCESSIBILITY_SUGGESTION: &str = "Open System Settings > Privacy & Security > Accessibility and add the app that launches agent-desktop, such as Terminal, iTerm, or Codex. If macOS lists the built binary separately, add that binary too.";
const SCREEN_RECORDING_SUGGESTION: &str = "Open System Settings > Privacy & Security > Screen Recording and add the app that launches agent-desktop, such as Terminal, iTerm, or Codex. If macOS lists the built binary separately, add that binary too.";

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
}

pub fn report() -> PermissionReport {
    PermissionReport {
        accessibility: accessibility_report_state(),
        screen_recording: screen_recording_report_state(),
        automation: PermissionState::NotRequired,
    }
}

pub fn request_report() -> PermissionReport {
    PermissionReport {
        accessibility: permission_state(imp::request_trust(), ACCESSIBILITY_SUGGESTION),
        screen_recording: permission_state(
            imp::request_screen_recording(),
            SCREEN_RECORDING_SUGGESTION,
        ),
        automation: PermissionState::NotRequired,
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
