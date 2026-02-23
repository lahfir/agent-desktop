use agent_desktop_core::error::AdapterError;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_foundation::base::TCFType;
    use std::ffi::c_void;

    type Id = *mut c_void;
    type Class = *mut c_void;
    type Sel = *mut c_void;

    extern "C" {
        fn objc_getClass(name: *const core::ffi::c_char) -> Class;
        fn sel_registerName(name: *const core::ffi::c_char) -> Sel;
        fn objc_msgSend(receiver: Id, sel: Sel, ...) -> Id;
        static NSPasteboardTypeString: Id;
    }

    fn pasteboard() -> Result<Id, AdapterError> {
        unsafe {
            let cls = objc_getClass(c"NSPasteboard".as_ptr());
            if cls.is_null() {
                return Err(AdapterError::internal("NSPasteboard class not found"));
            }
            let sel = sel_registerName(c"generalPasteboard".as_ptr());
            let send: unsafe extern "C" fn(Class, Sel) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);
            let pb = send(cls, sel);
            if pb.is_null() {
                return Err(AdapterError::internal("generalPasteboard returned null"));
            }
            Ok(pb)
        }
    }

    pub fn get() -> Result<String, AdapterError> {
        unsafe {
            let pb = pasteboard()?;
            let sel = sel_registerName(c"stringForType:".as_ptr());
            let send: unsafe extern "C" fn(Id, Sel, Id) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);
            let ns_string = send(pb, sel, NSPasteboardTypeString);
            if ns_string.is_null() {
                return Ok(String::new());
            }
            let cf_str = core_foundation::string::CFString::wrap_under_get_rule(
                ns_string as core_foundation_sys::string::CFStringRef,
            );
            Ok(cf_str.to_string())
        }
    }

    pub fn set(text: &str) -> Result<(), AdapterError> {
        unsafe {
            let pb = pasteboard()?;
            let clear_sel = sel_registerName(c"clearContents".as_ptr());
            let send_void: unsafe extern "C" fn(Id, Sel) =
                std::mem::transmute(objc_msgSend as *const c_void);
            send_void(pb, clear_sel);

            let cf_text = core_foundation::string::CFString::new(text);
            let ns_text = cf_text.as_concrete_TypeRef() as Id;
            let set_sel = sel_registerName(c"setString:forType:".as_ptr());
            let send_two: unsafe extern "C" fn(Id, Sel, Id, Id) -> bool =
                std::mem::transmute(objc_msgSend as *const c_void);
            let ok = send_two(pb, set_sel, ns_text, NSPasteboardTypeString);
            if !ok {
                return Err(AdapterError::internal(
                    "NSPasteboard setString:forType: failed",
                ));
            }
            Ok(())
        }
    }

    pub fn clear() -> Result<(), AdapterError> {
        unsafe {
            let pb = pasteboard()?;
            let sel = sel_registerName(c"clearContents".as_ptr());
            let send: unsafe extern "C" fn(Id, Sel) =
                std::mem::transmute(objc_msgSend as *const c_void);
            send(pb, sel);
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn get() -> Result<String, AdapterError> {
        Err(AdapterError::not_supported("clipboard_get"))
    }

    pub fn set(_text: &str) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("clipboard_set"))
    }

    pub fn clear() -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("clipboard_clear"))
    }
}

pub use imp::{clear, get, set};
