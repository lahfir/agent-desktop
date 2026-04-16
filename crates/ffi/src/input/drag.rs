use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::AdDragParams;
use crate::AdAdapter;
use agent_desktop_core::action::{DragParams as CoreDragParams, Point as CorePoint};

/// Synthesizes a mouse drag from `params.from` to `params.to`. When
/// `params.duration_ms` is zero the drag is instantaneous; a non-zero
/// value asks the platform adapter to interpolate.
///
/// # Safety
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `params` must be a non-null pointer to a valid `AdDragParams`.
#[no_mangle]
pub unsafe extern "C" fn ad_drag(
    adapter: *const AdAdapter,
    params: *const AdDragParams,
) -> AdResult {
    trap_panic(|| unsafe {
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(params, c"params is null");
        let adapter = &*adapter;
        let p = &*params;
        let core_params = CoreDragParams {
            from: CorePoint {
                x: p.from.x,
                y: p.from.y,
            },
            to: CorePoint {
                x: p.to.x,
                y: p.to.y,
            },
            duration_ms: if p.duration_ms == 0 {
                None
            } else {
                Some(p.duration_ms)
            },
        };
        match adapter.inner.drag(core_params) {
            Ok(()) => AdResult::Ok,
            Err(e) => {
                error::set_last_error(&e);
                error::last_error_code()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AdPoint;

    #[test]
    fn test_drag_zero_duration_becomes_none() {
        let p = AdDragParams {
            from: AdPoint { x: 0.0, y: 0.0 },
            to: AdPoint { x: 100.0, y: 200.0 },
            duration_ms: 0,
        };
        let core = CoreDragParams {
            from: CorePoint {
                x: p.from.x,
                y: p.from.y,
            },
            to: CorePoint {
                x: p.to.x,
                y: p.to.y,
            },
            duration_ms: if p.duration_ms == 0 {
                None
            } else {
                Some(p.duration_ms)
            },
        };
        assert!(core.duration_ms.is_none());
        assert_eq!(core.to.x, 100.0);
    }

    #[test]
    fn test_drag_nonzero_duration() {
        let p = AdDragParams {
            from: AdPoint { x: 0.0, y: 0.0 },
            to: AdPoint { x: 50.0, y: 50.0 },
            duration_ms: 500,
        };
        let core = CoreDragParams {
            from: CorePoint {
                x: p.from.x,
                y: p.from.y,
            },
            to: CorePoint {
                x: p.to.x,
                y: p.to.y,
            },
            duration_ms: if p.duration_ms == 0 {
                None
            } else {
                Some(p.duration_ms)
            },
        };
        assert_eq!(core.duration_ms, Some(500));
    }
}
