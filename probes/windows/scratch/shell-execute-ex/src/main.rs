//! Standalone ShellExecuteExW binding + launch scratch.
//! Prints shape-only JSON. Win32_UI_Shell stays out of crates/windows.

use std::env;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn main() {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };

    let symbol_ok = (ShellExecuteExW as usize) != 0;
    let target = env::args()
        .nth(1)
        .unwrap_or_else(|| env::var("COMSPEC").unwrap_or_else(|_| String::from("cmd.exe")));
    let mut file = wide(&target);
    let mut params = wide("/c exit 0");
    let mut sei: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    sei.fMask = SEE_MASK_NOCLOSEPROCESS;
    sei.lpFile = file.as_mut_ptr();
    sei.lpParameters = params.as_mut_ptr();
    sei.nShow = 0;

    let launched = unsafe { ShellExecuteExW(&mut sei) != 0 };
    let mut wait_signaled = false;
    let mut process_handle_nonzero = false;
    if launched && !sei.hProcess.is_null() {
        process_handle_nonzero = true;
        let wait = unsafe { WaitForSingleObject(sei.hProcess, 10_000) };
        wait_signaled = wait == WAIT_OBJECT_0;
        unsafe {
            CloseHandle(sei.hProcess);
        }
    }

    println!(
        "{{\"binding_ok\":{},\"launch_ok\":{},\"process_handle_nonzero\":{},\"wait_signaled\":{},\
\"win32_ui_shell_feature\":\"standalone_only\",\"ownership_note\":\"section_2_14_owns_by_name_aumid\",\
\"null_ptr_unused\":{}}}",
        symbol_ok,
        launched,
        process_handle_nonzero,
        wait_signaled,
        ptr::null::<u8>().is_null()
    );
}
