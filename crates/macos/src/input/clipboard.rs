use agent_desktop_core::error::AdapterError;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_foundation::base::TCFType;
    use std::ffi::c_void;

    type Id = *mut c_void;
    type Class = *mut c_void;
    type Sel = *mut c_void;

    unsafe extern "C" {
        fn objc_getClass(name: *const core::ffi::c_char) -> Class;
        fn sel_registerName(name: *const core::ffi::c_char) -> Sel;
        fn objc_msgSend(receiver: Id, sel: Sel, ...) -> Id;
        static NSPasteboardTypeString: Id;
    }

    pub(crate) struct ClipboardSnapshot {
        items: Id,
    }

    impl ClipboardSnapshot {
        pub(crate) fn capture() -> Result<Self, AdapterError> {
            unsafe {
                let pb = pasteboard()?;
                Ok(Self {
                    items: deep_copy_pasteboard_items(pb),
                })
            }
        }

        pub(crate) fn restore(&self) -> Result<(), AdapterError> {
            unsafe {
                let pb = pasteboard()?;
                clear_pasteboard(pb);
                if !self.items.is_null() && !write_objects(pb, self.items) {
                    tracing::warn!(
                        "clipboard restore failed after clearContents; original clipboard content is lost"
                    );
                    return Err(AdapterError::internal("NSPasteboard writeObjects: failed"));
                }
                Ok(())
            }
        }
    }

    impl Drop for ClipboardSnapshot {
        fn drop(&mut self) {
            unsafe { release_object(self.items) };
        }
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
        tracing::debug!("clipboard: get");
        unsafe {
            let pb = pasteboard()?;
            let Some(result) = read_string(pb) else {
                tracing::debug!("clipboard: get -> empty");
                return Ok(String::new());
            };
            tracing::debug!("clipboard: get -> {} chars", result.len());
            Ok(result)
        }
    }

    pub fn set(text: &str) -> Result<(), AdapterError> {
        tracing::debug!("clipboard: set {} chars", text.len());
        unsafe {
            let pb = pasteboard()?;
            let previous = ClipboardSnapshot::capture()?;
            clear_pasteboard(pb);
            if !write_string(pb, text) {
                let _ = previous.restore();
                return Err(AdapterError::internal(
                    "NSPasteboard setString:forType: failed",
                ));
            }
            Ok(())
        }
    }

    pub fn clear() -> Result<(), AdapterError> {
        tracing::debug!("clipboard: clear");
        unsafe {
            let pb = pasteboard()?;
            clear_pasteboard(pb);
            Ok(())
        }
    }

    unsafe fn read_string(pb: Id) -> Option<String> {
        unsafe {
            let sel = sel_registerName(c"stringForType:".as_ptr());
            let send: unsafe extern "C" fn(Id, Sel, Id) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);
            let ns_string = send(pb, sel, NSPasteboardTypeString);
            if ns_string.is_null() {
                return None;
            }
            let cf_str = core_foundation::string::CFString::wrap_under_get_rule(
                ns_string as core_foundation_sys::string::CFStringRef,
            );
            Some(cf_str.to_string())
        }
    }

    unsafe fn deep_copy_pasteboard_items(pb: Id) -> Id {
        unsafe {
            let alloc_sel = sel_registerName(c"alloc".as_ptr());
            let init_sel = sel_registerName(c"init".as_ptr());
            let send: unsafe extern "C" fn(Id, Sel) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);

            let ma_cls = objc_getClass(c"NSMutableArray".as_ptr());
            if ma_cls.is_null() {
                return std::ptr::null_mut();
            }
            let ma_alloc = send(ma_cls as Id, alloc_sel);
            if ma_alloc.is_null() {
                return std::ptr::null_mut();
            }
            let mutable_array = send(ma_alloc, init_sel);
            if mutable_array.is_null() {
                return std::ptr::null_mut();
            }

            let items_sel = sel_registerName(c"pasteboardItems".as_ptr());
            let pb_items = send(pb, items_sel);
            if pb_items.is_null() {
                release_object(mutable_array);
                return std::ptr::null_mut();
            }

            let count_sel = sel_registerName(c"count".as_ptr());
            let send_usize: unsafe extern "C" fn(Id, Sel) -> usize =
                std::mem::transmute(objc_msgSend as *const c_void);
            let item_count = send_usize(pb_items, count_sel);

            let idx_sel = sel_registerName(c"objectAtIndex:".as_ptr());
            let send_at_idx: unsafe extern "C" fn(Id, Sel, usize) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);

            let types_sel = sel_registerName(c"types".as_ptr());
            let data_sel = sel_registerName(c"dataForType:".as_ptr());
            let send_with_id: unsafe extern "C" fn(Id, Sel, Id) -> Id =
                std::mem::transmute(objc_msgSend as *const c_void);

            let set_data_sel = sel_registerName(c"setData:forType:".as_ptr());
            let send_set_data: unsafe extern "C" fn(Id, Sel, Id, Id) -> bool =
                std::mem::transmute(objc_msgSend as *const c_void);

            let add_sel = sel_registerName(c"addObject:".as_ptr());
            let send_add: unsafe extern "C" fn(Id, Sel, Id) =
                std::mem::transmute(objc_msgSend as *const c_void);

            let pbi_cls = objc_getClass(c"NSPasteboardItem".as_ptr());
            if pbi_cls.is_null() {
                release_object(mutable_array);
                return std::ptr::null_mut();
            }

            let mut added = false;
            for i in 0..item_count {
                let orig_item = send_at_idx(pb_items, idx_sel, i);
                if orig_item.is_null() {
                    continue;
                }

                let types = send(orig_item, types_sel);
                if types.is_null() {
                    continue;
                }

                let type_count = send_usize(types, count_sel);
                if type_count == 0 {
                    continue;
                }

                let fresh_alloc = send(pbi_cls as Id, alloc_sel);
                if fresh_alloc.is_null() {
                    continue;
                }
                let fresh_item = send(fresh_alloc, init_sel);
                if fresh_item.is_null() {
                    continue;
                }

                for j in 0..type_count {
                    let type_str = send_at_idx(types, idx_sel, j);
                    if type_str.is_null() {
                        continue;
                    }
                    let data = send_with_id(orig_item, data_sel, type_str);
                    if data.is_null() {
                        continue;
                    }
                    let _ = send_set_data(fresh_item, set_data_sel, data, type_str);
                }

                send_add(mutable_array, add_sel, fresh_item);
                release_object(fresh_item);
                added = true;
            }

            if !added {
                release_object(mutable_array);
                return std::ptr::null_mut();
            }
            mutable_array
        }
    }

    unsafe fn release_object(object: Id) {
        if object.is_null() {
            return;
        }
        unsafe {
            let sel = sel_registerName(c"release".as_ptr());
            let send: unsafe extern "C" fn(Id, Sel) =
                std::mem::transmute(objc_msgSend as *const c_void);
            send(object, sel);
        }
    }

    unsafe fn clear_pasteboard(pb: Id) {
        unsafe {
            let clear_sel = sel_registerName(c"clearContents".as_ptr());
            let send_void: unsafe extern "C" fn(Id, Sel) =
                std::mem::transmute(objc_msgSend as *const c_void);
            send_void(pb, clear_sel);
        }
    }

    unsafe fn write_string(pb: Id, text: &str) -> bool {
        unsafe {
            let cf_text = core_foundation::string::CFString::new(text);
            let ns_text = cf_text.as_concrete_TypeRef() as Id;
            let set_sel = sel_registerName(c"setString:forType:".as_ptr());
            let send_two: unsafe extern "C" fn(Id, Sel, Id, Id) -> bool =
                std::mem::transmute(objc_msgSend as *const c_void);
            send_two(pb, set_sel, ns_text, NSPasteboardTypeString)
        }
    }

    unsafe fn write_objects(pb: Id, objects: Id) -> bool {
        unsafe {
            let sel = sel_registerName(c"writeObjects:".as_ptr());
            let send: unsafe extern "C" fn(Id, Sel, Id) -> bool =
                std::mem::transmute(objc_msgSend as *const c_void);
            send(pb, sel, objects)
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

    pub(crate) struct ClipboardSnapshot;

    impl ClipboardSnapshot {
        pub(crate) fn capture() -> Result<Self, AdapterError> {
            Err(AdapterError::not_supported("clipboard_snapshot"))
        }

        pub(crate) fn restore(&self) -> Result<(), AdapterError> {
            Err(AdapterError::not_supported("clipboard_snapshot"))
        }
    }
}

pub(crate) use imp::ClipboardSnapshot;
pub use imp::{clear, get, set};
