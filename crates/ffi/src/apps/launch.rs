use crate::AdAdapter;
use crate::convert::window::{
    exact_window_info_to_c, validate_exact_window_info, window_info_to_c,
};
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::trap_panic;
use crate::types::{AdExactWindowInfo, AdWindowInfo};
use std::os::raw::c_char;

/// Launches the application identified by `id` (bundle id on macOS,
/// executable path on other platforms) and, on success, writes the
/// first window that becomes available into `*out`. Waits up to
/// `timeout_ms` for the window to appear; zero means "no wait".
///
/// The returned `AdWindowInfo` owns heap-allocated interior strings that
/// must be released with `ad_release_window_fields` once done. On error
/// the out-param is zero-initialized, so calling the release fn on it
/// is a safe no-op.
///
/// # Safety
/// `adapter` must be non-null. `id` must be a non-null UTF-8 C string.
/// `out` must be a non-null writable `*mut AdWindowInfo`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_launch_app(
    adapter: *const AdAdapter,
    id: *const c_char,
    timeout_ms: u64,
    out: *mut AdWindowInfo,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::mem::zeroed();
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let id_str = match super::decode_app_id(id) {
            Ok(id) => id,
            Err(err) => {
                set_last_error(&err);
                return crate::error::last_error_code();
            }
        };
        let adapter = crate::adapter::acquire_adapter!(adapter);
        let options = agent_desktop_core::launch_options::LaunchOptions {
            timeout_ms,
            ..Default::default()
        };
        let deadline = match launch_deadline(timeout_ms) {
            Ok(deadline) => deadline,
            Err(error) => {
                set_last_error(&error);
                return crate::error::last_error_code();
            }
        };
        let lease = match adapter.inner.acquire_interaction_lease(deadline) {
            Ok(lease) => lease,
            Err(error) => {
                set_last_error(&error);
                return crate::error::last_error_code();
            }
        };
        match adapter.inner.launch_app(&id_str, &options, &lease) {
            Ok(win) => {
                *out = window_info_to_c(&win);
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}

/// Launches an application and returns a generation-pinned exact window.
///
/// # Safety
/// `adapter`, `id`, and `out` must satisfy the same requirements as
/// `ad_launch_app`. Release the result with `ad_release_exact_window_fields`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_launch_app_exact(
    adapter: *const AdAdapter,
    id: *const c_char,
    timeout_ms: u64,
    out: *mut AdExactWindowInfo,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::mem::zeroed();
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let id = match super::decode_app_id(id) {
            Ok(id) => id,
            Err(error) => {
                set_last_error(&error);
                return crate::error::last_error_code();
            }
        };
        let adapter = crate::adapter::acquire_adapter!(adapter);
        let options = agent_desktop_core::launch_options::LaunchOptions {
            timeout_ms,
            ..Default::default()
        };
        let deadline = match launch_deadline(timeout_ms) {
            Ok(deadline) => deadline,
            Err(error) => {
                set_last_error(&error);
                return crate::error::last_error_code();
            }
        };
        let lease = match adapter.inner.acquire_interaction_lease(deadline) {
            Ok(lease) => lease,
            Err(error) => {
                set_last_error(&error);
                return crate::error::last_error_code();
            }
        };
        match adapter.inner.launch_app(&id, &options, &lease) {
            Ok(window) => match validate_exact_window_info(&window) {
                Ok(()) => {
                    *out = exact_window_info_to_c(&window);
                    AdResult::Ok
                }
                Err(error) => {
                    set_last_error(&error);
                    crate::error::last_error_code()
                }
            },
            Err(error) => {
                set_last_error(&error);
                crate::error::last_error_code()
            }
        }
    })
}

fn launch_deadline(
    timeout_ms: u64,
) -> Result<agent_desktop_core::Deadline, agent_desktop_core::AdapterError> {
    if timeout_ms == 0 {
        crate::operation::deadline()
    } else {
        agent_desktop_core::Deadline::after(timeout_ms)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    use agent_desktop_core::launch_options::LaunchOptions;
    use agent_desktop_core::{ActionOps, InputOps, ObservationOps, SystemOps};
    use agent_desktop_core::{
        AdapterError, Deadline, InteractionLease, ProcessId, WindowInfo, WindowState,
    };

    use super::*;

    struct LaunchProbe {
        calls: AtomicUsize,
        timeout_ms: AtomicU64,
    }

    struct LaunchAdapter {
        probe: Arc<LaunchProbe>,
    }

    impl ObservationOps for LaunchAdapter {}
    impl ActionOps for LaunchAdapter {}
    impl InputOps for LaunchAdapter {}

    impl SystemOps for LaunchAdapter {
        fn acquire_interaction_lease(
            &self,
            deadline: Deadline,
        ) -> Result<InteractionLease, AdapterError> {
            InteractionLease::guarded(deadline, ())
        }

        fn launch_app(
            &self,
            _id: &str,
            options: &LaunchOptions,
            _lease: &InteractionLease,
        ) -> Result<WindowInfo, AdapterError> {
            self.probe.calls.fetch_add(1, Ordering::SeqCst);
            self.probe
                .timeout_ms
                .store(options.timeout_ms, Ordering::SeqCst);
            Ok(WindowInfo {
                id: "w-launch".into(),
                title: "Launched".into(),
                app: "Fixture".into(),
                pid: ProcessId::new(42),
                process_instance: Some("fixture-42".into()),
                bounds: None,
                state: WindowState::default(),
            })
        }
    }

    #[test]
    fn zero_timeout_reaches_launch_once_without_defaulting() {
        let probe = Arc::new(LaunchProbe {
            calls: AtomicUsize::new(0),
            timeout_ms: AtomicU64::new(u64::MAX),
        });
        let adapter = crate::adapter::register_adapter(crate::AdAdapter {
            inner: Box::new(LaunchAdapter {
                probe: Arc::clone(&probe),
            }),
            session_id: None,
            _session_lease: None,
        })
        .unwrap();
        let id = CString::new("Fixture").unwrap();
        let mut out: AdWindowInfo = unsafe { std::mem::zeroed() };

        assert_eq!(
            unsafe { ad_launch_app(adapter, id.as_ptr(), 0, &mut out) },
            AdResult::Ok
        );
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        assert_eq!(probe.timeout_ms.load(Ordering::SeqCst), 0);

        unsafe {
            crate::windows::free_one::ad_release_window_fields(&mut out);
            crate::adapter::ad_adapter_destroy(adapter);
        }
    }
}
