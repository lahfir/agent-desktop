use agent_desktop_core::{
    AdapterError, ErrorCode, ObservationRequest, ObservationRoot, ObservedTree,
};
use serde_json::json;

use super::chromium;
use super::surfaces::surface_root;
use super::walker::{DEFAULT_MAX_RAW_DEPTH, WalkBudget};
use super::walker_source::walk_uia_subtree;

/// Runs one observation: resolve the root, walk it, detect a Chromium shell,
/// and report completeness honestly.
///
/// The walk itself already produces honest boundaries: at a logical or raw
/// depth boundary it has already enumerated the full child list (bounded by
/// the sibling and deadline budgets), so `children_count` is the real count and
/// `subtree_truncated` records that descendants were not walked - the two flags
/// set independently (KTD12's "a boundary node may carry a count with
/// `subtree_truncated: true`"). A deadline-starved walk reports the partial
/// tree it observed with `complete: false`, never as a discard.
///
/// This function adds what the walk cannot decide for itself:
///
/// 1. **Enumeration-failure surfacing**: a walk with real faults returns a
///    structured error, never a silently-truncated success.
/// 2. **Liveness-checked completeness** (KTD8): a walk that would report
///    `complete` is only trusted after the root re-verifies live; failure is
///    `WINDOW_NOT_FOUND`, never a complete-looking tree.
/// 3. **Chromium shell detection** (KTD7): a full-depth walk of a detected
///    Chromium root that lands on the pre-activation shell returns the
///    activation-required error, which core's loop settles and retries.
pub(crate) fn observe_tree(
    root: ObservationRoot<'_>,
    request: &ObservationRequest,
    adapter: &crate::adapter::WindowsAdapter,
) -> Result<ObservedTree, AdapterError> {
    let request = (*request).validate()?;
    let element = surface_root(root, request.surface, request.deadline)?;
    let chromium_root = chromium::is_chromium_root(&element);
    let budget = WalkBudget::new(request.max_logical_depth, request.deadline)
        .with_max_raw_depth(request.max_raw_depth.min(DEFAULT_MAX_RAW_DEPTH));
    let outcome = walk_uia_subtree(&element, &root, budget)?;

    if !outcome.failures.is_empty() {
        return Err(walk_fault_error(&outcome.failures));
    }
    if outcome.tree.is_complete()
        && chromium_root
        && chromium::activation_eligible(root, &request)
        && chromium::is_shell_shaped(&outcome.stats, &request)
    {
        if request.observation_mode.force_renderer_accessibility {
            // The caller already handled renderer accessibility (or is doing
            // so) via `--force-electron-a11y`: the flag's documented contract
            // is that the adapter "skips activation guidance" and returns the
            // tree it observed. Remove the activation-nag by handing back the
            // observed tree instead of the still-thin guidance error.
            return Ok(outcome.tree);
        }
        if adapter.renderer_activation_attempted() {
            return Err(still_thin_after_settle());
        }
        return Err(chromium::activation_required(&outcome.stats));
    }
    if outcome.tree.is_complete() {
        re_verify_root(root)?;
    }
    Ok(outcome.tree)
}

/// The post-settle still-thin error (KTD7): **not** marked activation-required,
/// so it escapes core's loop and reaches the caller - a Chromium tree that
/// genuinely stays thin after the async build has the guidance `platform_detail`
/// and no target-derived text.
fn still_thin_after_settle() -> AdapterError {
    agent_desktop_core::AdapterError::new(
        ErrorCode::ActionFailed,
        "The Chromium tree is still thin after its accessibility build settled",
    )
    .with_platform_detail(chromium::still_thin_detail())
}

/// The KTD8 liveness check: a walk that would claim `complete` only does so
/// after the root re-verifies live - `IsWindow` plus the process-generation
/// token (KTD3). A dead provider's sibling terminator is indistinguishable
/// from end-of-list (A14-4), so the terminator alone never proves
/// completeness; this independent read is what makes the claim honest.
fn re_verify_root(root: ObservationRoot<'_>) -> Result<(), AdapterError> {
    match root {
        ObservationRoot::Window(window) => {
            let handle = hwnd_of(&window.id)?;
            if !crate::tree::automation::window_exists(handle) {
                return Err(window_gone());
            }
            match window.process_instance.as_deref() {
                Some(instance) => {
                    if !crate::system::process_identity::matches_instance(window.pid, instance)? {
                        return Err(window_identity_changed());
                    }
                }
                // KTD3's fail-closed rule for the elevated/split-integrity
                // population: a window whose process token could not be read
                // (A16-12) can never be identity-corroborated, so the liveness
                // gate must not accept IsWindow alone against a potentially
                // recycled HWND.
                None => return Err(window_identity_changed()),
            }
            Ok(())
        }
        ObservationRoot::Element { .. } => Ok(()),
    }
}

fn hwnd_of(id: &str) -> Result<isize, AdapterError> {
    id.strip_prefix("w-")
        .and_then(|number| number.parse::<isize>().ok())
        .filter(|handle| *handle > 0)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Malformed window identifier"))
}

fn window_gone() -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "The window was destroyed during observation",
    )
    .with_suggestion("Run 'list-windows' to refresh window IDs, then retry.")
}

fn window_identity_changed() -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "The window's process changed during observation",
    )
    .with_suggestion("Run 'list-windows' to refresh window IDs, then retry.")
}

/// Builds the structured error a walk with real enumeration faults surfaces.
///
/// Shape only: each fault's kind, axis, depth, and child index - never an
/// application name or property value, because this error reaches trace
/// segments.
fn walk_fault_error(failures: &[AdapterError]) -> AdapterError {
    let faults: Vec<serde_json::Value> = failures
        .iter()
        .map(|failure| {
            failure
                .details
                .clone()
                .unwrap_or(json!({ "kind": "walk_fault" }))
        })
        .collect();
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Element tree walk encountered an enumeration failure",
    )
    .with_details(json!({ "faults": faults }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_malformed_root_window_id_fails_before_walking() {
        let request = ObservationRequest::snapshot(
            &agent_desktop_core::TreeOptions::default(),
            agent_desktop_core::Deadline::after(5_000).unwrap(),
        );
        let window = agent_desktop_core::WindowInfo {
            id: "not-a-window-id".into(),
            title: "x".into(),
            app: "x".into(),
            pid: agent_desktop_core::ProcessId::new(1),
            process_instance: None,
            bounds: None,
            state: Default::default(),
        };

        let error = observe_tree(
            ObservationRoot::Window(&window),
            &request,
            &crate::adapter::WindowsAdapter::new(),
        )
        .expect_err("a malformed id must fail");
        assert_eq!(error.code, ErrorCode::InvalidArgs);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_destroyed_hwnd_fails_reverification_as_window_not_found() {
        let window = agent_desktop_core::WindowInfo {
            id: format!("w-{}", 0x7fffffff),
            title: "x".into(),
            app: "x".into(),
            pid: agent_desktop_core::ProcessId::new(1),
            process_instance: None,
            bounds: None,
            state: Default::default(),
        };

        let error = re_verify_root(agent_desktop_core::ObservationRoot::Window(&window))
            .expect_err("a destroyed handle must fail the liveness gate");
        assert_eq!(error.code, ErrorCode::WindowNotFound);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_freshly_created_fixture_window_passes_reverification() {
        use agent_desktop_core::ObservationRoot;

        crate::tree::fixture::ensure_test_apartment();
        let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
        let window = agent_desktop_core::WindowInfo {
            id: format!("w-{}", fixture.handle()),
            title: "agent-desktop fixture".into(),
            app: "fixture.exe".into(),
            pid: agent_desktop_core::ProcessId::from(fixture.process_id()),
            process_instance: Some(
                crate::system::process_identity::token_for_pid(
                    agent_desktop_core::ProcessId::from(fixture.process_id()),
                )
                .unwrap()
                .expect("a live fixture process has a token"),
            ),
            bounds: None,
            state: Default::default(),
        };

        let result = re_verify_root(ObservationRoot::Window(&window));
        assert!(
            result.is_ok(),
            "a live fixture root must pass the liveness gate"
        );
    }
}
