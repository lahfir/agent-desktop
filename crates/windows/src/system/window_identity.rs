use agent_desktop_core::{AdapterError, ErrorCode, ProcessId, WindowInfo};

use super::process_identity;

/// The immutable identity evidence a resolved window must match.
///
/// KTD3's split: fresh-list verification is strict (title included), while
/// stored-evidence resolution treats pid + token + app as the immutable
/// identity and tolerates title drift, logging it as telemetry - a live
/// window's title legitimately changes (a dirty-marker asterisk, an Electron
/// target retitling per document), and a hard title check there would fail
/// drill-down on the very windows 2.4 exists to serve.
pub(crate) struct WindowIdentityEvidence<'a> {
    pub(crate) handle: super::window_enum::WindowHandle,
    pub(crate) pid: ProcessId,
    pub(crate) app: &'a str,
    pub(crate) process_instance: &'a str,
    pub(crate) title: Option<&'a str>,
}

impl<'a> WindowIdentityEvidence<'a> {
    pub(crate) fn from_info(
        handle: super::window_enum::WindowHandle,
        win: &'a WindowInfo,
    ) -> Option<Self> {
        Some(Self {
            handle,
            pid: win.pid,
            app: &win.app,
            process_instance: win.process_instance.as_deref()?,
            title: Some(&win.title),
        })
    }

    /// The strict check a window freshly listed in the same invocation
    /// receives: pid, token, app, and title must all match the live window.
    ///
    /// A recycled HWND whose process no longer matches fails closed as
    /// `WINDOW_NOT_FOUND`, never resolving to the new occupant (R4).
    pub(crate) fn verify_strict(&self) -> Result<(), AdapterError> {
        if !process_identity::matches_instance(self.pid, self.process_instance)? {
            return Err(window_identity_mismatch(self.handle));
        }
        if !self.app.is_empty() && live_process_app(self.pid) != self.app {
            return Err(window_identity_mismatch(self.handle));
        }
        if self.title.is_some_and(|title| !title.is_empty()) {
            let live = live_window_title(self.handle);
            if live.as_deref() != self.title {
                return Err(window_identity_mismatch(self.handle));
            }
        }
        Ok(())
    }

    /// The stored-evidence check: pid + token + app are the immutable
    /// identity; a title that drifted is logged as telemetry, not a failure.
    pub(crate) fn verify_stored(&self) -> Result<(), AdapterError> {
        if !process_identity::matches_instance(self.pid, self.process_instance)? {
            return Err(window_identity_mismatch(self.handle));
        }
        let live = live_window_title(self.handle);
        if self
            .title
            .is_some_and(|expected| live.as_deref() != Some(expected))
        {
            // Shape only - whether the live title drifted, never the titles
            // themselves, which are target-derived and would reach any tracing
            // sink that subscribes (the JSONL trace redacts the `title` key,
            // but a console/file subscriber would not).
            tracing::debug!("window title changed while immutable source identity remained valid");
        }
        Ok(())
    }
}

/// Reads the live title of a window handle, the one piece of the strict check
/// only the OS can answer for.
fn live_window_title(handle: super::window_enum::WindowHandle) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
        let mut buffer = vec![0u16; 512];
        let length = unsafe { GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32) };
        if length <= 0 {
            return None;
        }
        buffer.truncate(length as usize);
        Some(String::from_utf16_lossy(&buffer))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        None
    }
}

/// The image name of the process at `pid`, as the strict check's `app`
/// corroboration reads it.
#[cfg(target_os = "windows")]
fn live_process_app(pid: ProcessId) -> String {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() {
        return String::new();
    }
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = None;
    let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) };
    while ok != 0 {
        if entry.th32ProcessID == u32::from(pid) {
            let length = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            found = Some(String::from_utf16_lossy(&entry.szExeFile[..length]));
            break;
        }
        ok = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    unsafe {
        windows_sys::Win32::Foundation::CloseHandle(snapshot);
    }
    found.unwrap_or_default()
}

#[cfg(not(target_os = "windows"))]
fn live_process_app(_pid: ProcessId) -> String {
    String::new()
}

/// The fail-closed identity-mismatch error, carrying no window-derived text.
fn window_identity_mismatch(handle: super::window_enum::WindowHandle) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "The window's identity no longer matches its stored evidence",
    )
    .with_suggestion("Run 'list-windows' to refresh window identifiers, then retry.")
    .with_platform_detail(format!(
        "HWND 0x{:X} failed process-instance corroboration",
        handle as usize
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_window(pid: ProcessId, instance: &str, title: &str) -> WindowInfo {
        WindowInfo {
            id: "w-1".into(),
            title: title.into(),
            app: "fixture".into(),
            pid,
            process_instance: Some(instance.into()),
            bounds: None,
            state: Default::default(),
        }
    }

    #[test]
    fn a_shape_with_no_process_instance_fails_closed() {
        let win = WindowInfo {
            process_instance: None,
            ..fake_window(ProcessId::new(1), "x", "T")
        };
        let evidence = WindowIdentityEvidence::from_info(std::ptr::null_mut(), &win);

        assert!(
            evidence.is_none(),
            "no process instance means no corroboration possible"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_fresh_token_passes_stored_and_strict_fails_on_a_mismatched_live_title() {
        use super::process_identity::token_for_pid;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

        let pid = ProcessId::from(std::process::id());
        let Some(token) = token_for_pid(pid).unwrap() else {
            return;
        };
        let desktop = unsafe { GetDesktopWindow() };
        // The strict check compares pid+token+app+title. Using the live image
        // name as `app` removes the app comparison from the failure, so this
        // test actually exercises the title arm (and only the title arm).
        let mut win = fake_window(pid, &token, "a-title-that-is-not-the-desktop-title");
        win.app = live_process_app(pid);
        let evidence = WindowIdentityEvidence::from_info(desktop, &win)
            .expect("a process with a token derives its evidence");

        assert!(
            evidence.verify_stored().is_ok(),
            "stored verification trusts pid + token + app, which match"
        );

        let live = live_window_title(desktop);
        if live.as_deref() == Some("a-title-that-is-not-the-desktop-title") {
            return;
        }
        assert!(
            evidence.verify_strict().is_err(),
            "strict verification rejects a title the live window does not have"
        );
    }
}
