use core_graphics::event::CGEvent;
use core_graphics::geometry::CGPoint;
use foreign_types::ForeignType;
use std::ffi::{CStr, c_void};
use std::sync::OnceLock;

/// Carbon process serial number, the process handle SkyLight's process APIs
/// take instead of a pid.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProcessSerialNumber {
    high: u32,
    low: u32,
}

type PostToPid = unsafe extern "C" fn(libc::pid_t, *mut c_void);
type SetWindowLocation = unsafe extern "C" fn(*mut c_void, CGPoint);
type PostEventRecordTo = unsafe extern "C" fn(*const ProcessSerialNumber, *const u8) -> i32;
type GetFrontProcess = unsafe extern "C" fn(*mut ProcessSerialNumber) -> i32;
type SetFrontProcessWithOptions = unsafe extern "C" fn(*const ProcessSerialNumber, u32, u32) -> i32;
type GetProcessForPid = unsafe extern "C" fn(libc::pid_t, *mut ProcessSerialNumber) -> i32;
type GetProcessPid = unsafe extern "C" fn(*const ProcessSerialNumber, *mut libc::pid_t) -> i32;

const SKYLIGHT_PATH: &CStr = c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";

/// yabai's `kCPSNoWindows`: make a process frontmost without raising or
/// reordering any of its windows.
const CPS_NO_WINDOWS: u32 = 0x400;

/// Private SkyLight and deprecated Carbon entry points, resolved at runtime
/// so a macOS release that drops one degrades that layer instead of failing
/// to load the binary. Signatures follow yabai, cua, and
/// background-computer-use, which call the same symbols; yabai spells the
/// front-process calls with a leading underscore and cua without, so both
/// spellings are tried.
struct Symbols {
    post_to_pid: Option<PostToPid>,
    set_window_location: Option<SetWindowLocation>,
    post_event_record: Option<PostEventRecordTo>,
    get_front_process: Option<GetFrontProcess>,
    set_front_process: Option<SetFrontProcessWithOptions>,
    process_for_pid: Option<GetProcessForPid>,
    pid_for_process: Option<GetProcessPid>,
}

fn symbols() -> &'static Symbols {
    static SYMBOLS: OnceLock<Symbols> = OnceLock::new();
    SYMBOLS.get_or_init(load_symbols)
}

/// The SkyLight handle is intentionally never closed: resolved function
/// pointers must stay valid for the life of the process.
fn load_symbols() -> Symbols {
    unsafe {
        libc::dlopen(SKYLIGHT_PATH.as_ptr(), libc::RTLD_LAZY);
        Symbols {
            post_to_pid: lookup(c"SLEventPostToPid"),
            set_window_location: lookup(c"CGEventSetWindowLocation"),
            post_event_record: lookup(c"SLPSPostEventRecordTo"),
            get_front_process: lookup(c"_SLPSGetFrontProcess")
                .or_else(|| lookup(c"SLPSGetFrontProcess")),
            set_front_process: lookup(c"_SLPSSetFrontProcessWithOptions")
                .or_else(|| lookup(c"SLPSSetFrontProcessWithOptions")),
            process_for_pid: lookup(c"GetProcessForPID"),
            pid_for_process: lookup(c"GetProcessPID"),
        }
    }
}

/// # Safety
///
/// `T` must be the `unsafe extern "C" fn` type matching the C signature of
/// the exported symbol `name`; the pointer is reinterpreted as that type.
unsafe fn lookup<T: Copy>(name: &CStr) -> Option<T> {
    if std::mem::size_of::<T>() != std::mem::size_of::<*mut c_void>() {
        return None;
    }
    let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    if symbol.is_null() {
        return None;
    }
    Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&symbol) })
}

/// Posts through `SLEventPostToPid`; `false` means the symbol is missing and
/// nothing was posted, so the caller may fall back without double delivery.
pub(crate) fn post_to_pid(pid: libc::pid_t, event: &CGEvent) -> bool {
    let Some(post) = symbols().post_to_pid else {
        return false;
    };
    unsafe { post(pid, event.as_ptr().cast()) };
    true
}

/// Sets the window-local location AppKit reports as `locationInWindow`;
/// `false` means `CGEventSetWindowLocation` is unavailable.
pub(crate) fn set_window_location(event: &CGEvent, location: CGPoint) -> bool {
    let Some(set) = symbols().set_window_location else {
        return false;
    };
    unsafe { set(event.as_ptr().cast(), location) };
    true
}

pub(crate) fn process_serial_number(pid: libc::pid_t) -> Option<ProcessSerialNumber> {
    let lookup = symbols().process_for_pid?;
    let mut psn = ProcessSerialNumber::default();
    (unsafe { lookup(pid, &mut psn) } == 0).then_some(psn)
}

/// `None` when `SLPSPostEventRecordTo` is missing, otherwise its status.
pub(crate) fn post_event_record(psn: &ProcessSerialNumber, record: &[u8]) -> Option<i32> {
    let post = symbols().post_event_record?;
    Some(unsafe { post(psn, record.as_ptr()) })
}

/// The window server's frontmost process, which stays current in a CLI that
/// never spins an AppKit run loop.
pub(crate) fn front_process_pid() -> Option<libc::pid_t> {
    let symbols = symbols();
    let (get_front, pid_for) = (symbols.get_front_process?, symbols.pid_for_process?);
    let mut psn = ProcessSerialNumber::default();
    if unsafe { get_front(&mut psn) } != 0 {
        return None;
    }
    let mut pid: libc::pid_t = 0;
    (unsafe { pid_for(&psn, &mut pid) } == 0 && pid > 0).then_some(pid)
}

/// Makes `pid` frontmost again without raising any of its windows. Only ever
/// called for the user's own previously frontmost app, never the target.
/// `None` when the symbols are missing, otherwise whether it succeeded.
pub(crate) fn restore_front_process(pid: libc::pid_t) -> Option<bool> {
    let set_front = symbols().set_front_process?;
    let psn = process_serial_number(pid)?;
    Some(unsafe { set_front(&psn, 0, CPS_NO_WINDOWS) } == 0)
}
