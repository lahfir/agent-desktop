use agent_desktop_core::{AdapterError, Deadline, ErrorCode, ObservationRoot};
use serde_json::json;

use super::automation::root_from_hwnd;
use super::element::{UIAElement, uia_element};

/// Resolves an observation root to the UI Automation element the walk starts
/// from (KTD2).
///
/// `ObservationRoot::Window` parses the window id back to an HWND, re-verifies
/// the process-generation token (KTD3), and enters the shipped hardened path -
/// `root_from_hwnd`, which already carries the `IsWindow` dead-handle check
/// (A14-5) and the `SendMessageTimeoutW(WM_NULL, SMTO_ABORTIFHUNG)` hang probe
/// (A14-11). No second HWND-consuming path is built.
///
/// `ObservationRoot::Element` passes through the stored `NativeHandle`.
pub(crate) fn resolve_root(
    root: ObservationRoot<'_>,
    deadline: Deadline,
) -> Result<UIAElement, AdapterError> {
    match root {
        ObservationRoot::Window(window) => {
            let handle = parse_window_id(&window.id)?;
            if let Some(instance) = window.process_instance.as_deref() {
                if !crate::system::process_identity::matches_instance(window.pid, instance)? {
                    return Err(window_identity_mismatch(&window.id));
                }
            }
            root_from_hwnd(handle, deadline)
        }
        ObservationRoot::Element { handle, entry, .. } => {
            let element = uia_element(handle).map_err(|_| {
                AdapterError::new(
                    ErrorCode::StaleRef,
                    "Element root handle is invalid for this platform",
                )
                .with_details(json!({
                    "kind": "element_root_wrong_payload",
                    "process": entry.process.pid,
                }))
            })?;
            Ok(element.clone())
        }
    }
}

/// Parses a `"w-<hwnd>"` window id back to the HWND the platform resolver
/// consumes. A non-numeric or zero id is an invalid window id.
fn parse_window_id(id: &str) -> Result<isize, AdapterError> {
    id.strip_prefix("w-")
        .and_then(|number| number.parse::<isize>().ok())
        .filter(|handle| *handle > 0)
        .ok_or_else(|| invalid_window_id(id))
}

fn invalid_window_id(id: &str) -> AdapterError {
    AdapterError::new(ErrorCode::InvalidArgs, "Malformed window identifier")
        .with_details(json!({ "kind": "malformed_window_id", "window_id": id }))
}

fn window_identity_mismatch(id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "The window's identity no longer matches its stored evidence",
    )
    .with_details(json!({ "kind": "window_identity_mismatch", "window_id": id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_malformed_window_id_is_an_invalid_argument() {
        for id in ["", "e1", "w-", "w-minus", "w-0", "w--5"] {
            let error = parse_window_id(id).expect_err("must reject a malformed id");
            assert_eq!(error.code, ErrorCode::InvalidArgs, "for id {id:?}");
        }
    }

    #[test]
    fn a_well_formed_hwnd_id_parses() {
        assert_eq!(parse_window_id("w-1234").unwrap(), 1234);
    }
}
