use agent_desktop_core::node::AppInfo;
use core_foundation::base::CFTypeRef;
use std::{ffi::c_void, sync::OnceLock};

type Id = *mut c_void;
type Class = *mut c_void;
type Sel = *mut c_void;
const RTLD_LAZY: i32 = 1;

struct AutoreleasePool(Option<Id>);

impl AutoreleasePool {
    unsafe fn new() -> Self {
        let pool = unsafe { objc_autoreleasePoolPush() };
        Self((!pool.is_null()).then_some(pool))
    }
}

impl Drop for AutoreleasePool {
    fn drop(&mut self) {
        if let Some(pool) = self.0 {
            unsafe { objc_autoreleasePoolPop(pool) };
        }
    }
}

pub(crate) fn list_apps() -> Vec<AppInfo> {
    if !appkit_loaded() {
        return Vec::new();
    }

    unsafe {
        let _pool = AutoreleasePool::new();
        let workspace_cls = objc_getClass(c"NSWorkspace".as_ptr());
        if workspace_cls.is_null() {
            return Vec::new();
        }

        let shared_sel = sel_registerName(c"sharedWorkspace".as_ptr());
        let send_class: unsafe extern "C" fn(Class, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let workspace = send_class(workspace_cls, shared_sel);
        if workspace.is_null() {
            return Vec::new();
        }

        let running_sel = sel_registerName(c"runningApplications".as_ptr());
        let send_id: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let running = send_id(workspace, running_sel);
        if running.is_null() {
            return Vec::new();
        }

        apps_from_running_array(running, send_id)
    }
}

fn appkit_loaded() -> bool {
    static APPKIT_LOADED: OnceLock<bool> = OnceLock::new();
    *APPKIT_LOADED.get_or_init(|| unsafe {
        !dlopen(
            c"/System/Library/Frameworks/AppKit.framework/AppKit".as_ptr(),
            RTLD_LAZY,
        )
        .is_null()
    })
}

fn apps_from_running_array(
    running: Id,
    send_id: unsafe extern "C" fn(Id, Sel) -> Id,
) -> Vec<AppInfo> {
    unsafe {
        let count_sel = sel_registerName(c"count".as_ptr());
        let send_count: unsafe extern "C" fn(Id, Sel) -> usize =
            std::mem::transmute(objc_msgSend as *const c_void);
        let count = send_count(running, count_sel);

        let object_sel = sel_registerName(c"objectAtIndex:".as_ptr());
        let send_object: unsafe extern "C" fn(Id, Sel, usize) -> Id =
            std::mem::transmute(objc_msgSend as *const c_void);
        let policy_sel = sel_registerName(c"activationPolicy".as_ptr());
        let send_policy: unsafe extern "C" fn(Id, Sel) -> isize =
            std::mem::transmute(objc_msgSend as *const c_void);
        let pid_sel = sel_registerName(c"processIdentifier".as_ptr());
        let send_pid: unsafe extern "C" fn(Id, Sel) -> i32 =
            std::mem::transmute(objc_msgSend as *const c_void);
        let name_sel = sel_registerName(c"localizedName".as_ptr());
        let bundle_sel = sel_registerName(c"bundleIdentifier".as_ptr());

        let mut seen_pids = rustc_hash::FxHashSet::default();
        let mut apps = Vec::new();
        for idx in 0..count {
            let app = send_object(running, object_sel, idx);
            if app.is_null() || send_policy(app, policy_sel) != 0 {
                continue;
            }

            let pid = send_pid(app, pid_sel);
            if pid <= 0 || !seen_pids.insert(pid) {
                continue;
            }

            if let Some(name) = ns_string(send_id(app, name_sel)) {
                apps.push(AppInfo {
                    name,
                    pid,
                    bundle_id: ns_string(send_id(app, bundle_sel)),
                });
            }
        }

        apps
    }
}

unsafe fn ns_string(id: Id) -> Option<String> {
    if id.is_null() {
        return None;
    }
    crate::cf_type::borrowed_cf_string(id as CFTypeRef).map(|value| value.to_string())
}

unsafe extern "C" {
    fn objc_autoreleasePoolPush() -> Id;
    fn objc_autoreleasePoolPop(pool: Id);
    fn dlopen(filename: *const core::ffi::c_char, flag: i32) -> Id;
    fn objc_getClass(name: *const core::ffi::c_char) -> Class;
    fn sel_registerName(name: *const core::ffi::c_char) -> Sel;
    fn objc_msgSend(receiver: Id, sel: Sel, ...) -> Id;
}

#[cfg(test)]
#[path = "workspace_apps_tests.rs"]
mod tests;
