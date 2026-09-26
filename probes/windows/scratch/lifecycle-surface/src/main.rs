//! Compile-check: lifecycle Win32 surface under crates/windows feature set.
//! Never a workspace member. Prints shape-only JSON for the probe to capture.

fn main() {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, GetExitCodeProcess, OpenProcess, TerminateProcess, WaitForSingleObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowPlacement, GetWindowRect, IsHungAppWindow,
        SendMessageTimeoutW, SetForegroundWindow, SetWindowPos, ShowWindow,
    };

    let symbols: [usize; 13] = [
        CreateProcessW as usize,
        TerminateProcess as usize,
        GetExitCodeProcess as usize,
        WaitForSingleObject as usize,
        OpenProcess as usize,
        CloseHandle as usize,
        SetWindowPos as usize,
        ShowWindow as usize,
        GetWindowPlacement as usize,
        GetWindowRect as usize,
        IsHungAppWindow as usize,
        SendMessageTimeoutW as usize,
        SetForegroundWindow as usize,
    ];
    let all_nonzero = symbols.iter().all(|a| *a != 0);
    let _ = GetForegroundWindow as usize;

    println!(
        "{{\"surface_compiles\":{},\"feature_set\":\"crates_windows_current\",\"symbol_count\":{},\"apis\":[\
\"CreateProcessW\",\"TerminateProcess\",\"GetExitCodeProcess\",\"WaitForSingleObject\",\
\"OpenProcess\",\"CloseHandle\",\"SetWindowPos\",\"ShowWindow\",\"GetWindowPlacement\",\"GetWindowRect\",\
\"IsHungAppWindow\",\"SendMessageTimeoutW\",\"SetForegroundWindow\",\"GetForegroundWindow\"]}}",
        all_nonzero,
        symbols.len()
    );
}
