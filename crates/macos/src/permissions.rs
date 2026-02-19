use agent_desktop_core::adapter::PermissionStatus;

#[cfg(target_os = "macos")]
mod imp {
    use accessibility_sys::{
        kAXTrustedCheckOptionPrompt, AXIsProcessTrusted, AXIsProcessTrustedWithOptions,
    };
    use core_foundation::{
        base::TCFType,
        boolean::CFBoolean,
        dictionary::CFDictionary,
        string::CFString,
    };

    pub fn is_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    pub fn request_trust() -> bool {
        unsafe {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let val = CFBoolean::true_value();
            let dict =
                CFDictionary::from_CFType_pairs(&[(key.as_CFType(), val.as_CFType())]);
            AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef())
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
}

pub fn check() -> PermissionStatus {
    if imp::is_trusted() {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied {
            suggestion: "Open System Settings > Privacy & Security > Accessibility and add your terminal application".into(),
        }
    }
}

pub fn check_with_request() -> PermissionStatus {
    let trusted = imp::request_trust();
    if trusted {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied {
            suggestion: "Grant accessibility permission in the system dialog that appeared".into(),
        }
    }
}
