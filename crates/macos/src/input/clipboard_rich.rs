use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use core_foundation_sys::base::kCFAllocatorDefault;
use core_foundation_sys::url::CFURLCreateWithString;
use std::ffi::c_void;

type Id = *mut c_void;
type Class = *mut c_void;
type Sel = *mut c_void;

unsafe extern "C" {
    fn objc_getClass(name: *const core::ffi::c_char) -> Class;
    fn sel_registerName(name: *const core::ffi::c_char) -> Sel;
    fn objc_msgSend(receiver: Id, sel: Sel, ...) -> Id;
    static NSPasteboardTypePNG: Id;
    static NSPasteboardTypeFileURL: Id;
}

/// Reads the width/height stored in a PNG's `IHDR` chunk directly from the
/// byte buffer, mirroring `macos::system::screenshot::png_dimensions` — no
/// image-decoding dependency needed for two big-endian `u32` reads.
pub(crate) fn png_dimensions(data: &[u8]) -> (u32, u32) {
    if data.len() < 24 {
        return (0, 0);
    }
    let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    (w, h)
}

pub(crate) fn read_image(pb: Id) -> Option<Vec<u8>> {
    unsafe { read_data(pb, NSPasteboardTypePNG) }
}

pub(crate) fn write_image(pb: Id, bytes: &[u8]) -> bool {
    unsafe { write_data(pb, bytes, NSPasteboardTypePNG) }
}

unsafe fn read_data(pb: Id, pasteboard_type: Id) -> Option<Vec<u8>> {
    unsafe {
        let sel = sel_registerName(c"dataForType:".as_ptr());
        let send: unsafe extern "C" fn(Id, Sel, Id) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let data = send(pb, sel, pasteboard_type);
        if data.is_null() {
            return None;
        }
        let length_sel = sel_registerName(c"length".as_ptr());
        let send_usize: unsafe extern "C" fn(Id, Sel) -> usize =
            std::mem::transmute(objc_msgSend as *const c_void);
        let len = send_usize(data, length_sel);
        if len == 0 {
            return Some(Vec::new());
        }
        let bytes_sel = sel_registerName(c"bytes".as_ptr());
        let send_ptr: unsafe extern "C" fn(Id, Sel) -> *const u8 =
            std::mem::transmute(objc_msgSend as *const c_void);
        let ptr = send_ptr(data, bytes_sel);
        if ptr.is_null() {
            return None;
        }
        Some(std::slice::from_raw_parts(ptr, len).to_vec())
    }
}

unsafe fn write_data(pb: Id, bytes: &[u8], pasteboard_type: Id) -> bool {
    unsafe {
        let cls = objc_getClass(c"NSData".as_ptr());
        if cls.is_null() {
            return false;
        }
        let data_sel = sel_registerName(c"dataWithBytes:length:".as_ptr());
        let send_new: unsafe extern "C" fn(Id, Sel, *const u8, usize) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let data = send_new(cls as Id, data_sel, bytes.as_ptr(), bytes.len());
        if data.is_null() {
            return false;
        }
        let set_sel = sel_registerName(c"setData:forType:".as_ptr());
        let send_set: unsafe extern "C" fn(Id, Sel, Id, Id) -> bool =
            std::mem::transmute(objc_msgSend as *const c_void);
        send_set(pb, set_sel, data, pasteboard_type)
    }
}

pub(crate) fn read_file_urls(pb: Id) -> Vec<String> {
    unsafe { read_file_urls_inner(pb) }
}

unsafe fn read_file_urls_inner(pb: Id) -> Vec<String> {
    unsafe {
        let items_sel = sel_registerName(c"pasteboardItems".as_ptr());
        let send: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let items = send(pb, items_sel);
        if items.is_null() {
            return Vec::new();
        }
        let count_sel = sel_registerName(c"count".as_ptr());
        let send_usize: unsafe extern "C" fn(Id, Sel) -> usize =
            std::mem::transmute(objc_msgSend as *const c_void);
        let count = send_usize(items, count_sel);

        let idx_sel = sel_registerName(c"objectAtIndex:".as_ptr());
        let send_at: unsafe extern "C" fn(Id, Sel, usize) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let string_sel = sel_registerName(c"stringForType:".as_ptr());
        let send_string: unsafe extern "C" fn(Id, Sel, Id) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);

        let mut urls = Vec::new();
        for i in 0..count {
            let item = send_at(items, idx_sel, i);
            if item.is_null() {
                continue;
            }
            let ns_url_string = send_string(item, string_sel, NSPasteboardTypeFileURL);
            if ns_url_string.is_null() {
                continue;
            }
            let url_string = CFString::wrap_under_get_rule(
                ns_url_string as core_foundation_sys::string::CFStringRef,
            )
            .to_string();
            if let Some(path) = file_url_to_path(&url_string) {
                urls.push(path);
            }
        }
        urls
    }
}

pub(crate) fn file_url_to_path(url_string: &str) -> Option<String> {
    if !url_string.starts_with("file://") {
        return None;
    }
    let cf_string = CFString::new(url_string);
    let url_ref = unsafe {
        CFURLCreateWithString(
            kCFAllocatorDefault,
            cf_string.as_concrete_TypeRef(),
            std::ptr::null(),
        )
    };
    if url_ref.is_null() {
        return None;
    }
    let url: CFURL = unsafe { TCFType::wrap_under_create_rule(url_ref) };
    url.to_path().map(|p| p.to_string_lossy().into_owned())
}

pub(crate) fn write_file_urls(pb: Id, paths: &[String]) -> bool {
    unsafe { write_file_urls_inner(pb, paths) }
}

unsafe fn write_file_urls_inner(pb: Id, paths: &[String]) -> bool {
    unsafe {
        let ma_cls = objc_getClass(c"NSMutableArray".as_ptr());
        if ma_cls.is_null() {
            return false;
        }
        let alloc_sel = sel_registerName(c"alloc".as_ptr());
        let init_sel = sel_registerName(c"init".as_ptr());
        let send: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let array = send(send(ma_cls as Id, alloc_sel), init_sel);
        if array.is_null() {
            return false;
        }

        let add_sel = sel_registerName(c"addObject:".as_ptr());
        let send_add: unsafe extern "C" fn(Id, Sel, Id) =
            std::mem::transmute(objc_msgSend as *const c_void);

        let mut added = false;
        for path in paths {
            let Some(url) = CFURL::from_path(path, false) else {
                continue;
            };
            let url_id = url.as_concrete_TypeRef() as *mut c_void;
            send_add(array, add_sel, url_id);
            added = true;
        }
        if !added {
            release(array);
            return false;
        }

        let write_sel = sel_registerName(c"writeObjects:".as_ptr());
        let send_write: unsafe extern "C" fn(Id, Sel, Id) -> bool =
            std::mem::transmute(objc_msgSend as *const c_void);
        let ok = send_write(pb, write_sel, array);
        release(array);
        ok
    }
}

unsafe fn release(object: Id) {
    unsafe {
        let sel = sel_registerName(c"release".as_ptr());
        let send: unsafe extern "C" fn(Id, Sel) =
            std::mem::transmute(objc_msgSend as *const c_void);
        send(object, sel);
    }
}

#[cfg(test)]
#[path = "clipboard_rich_tests.rs"]
mod tests;
