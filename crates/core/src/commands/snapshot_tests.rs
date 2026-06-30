use super::*;
use crate::adapter::{PlatformAdapter, WindowFilter};
use crate::context::{CommandContext, WaitSelector};
use crate::error::{AdapterError, ErrorCode};
use crate::node::{AccessibilityNode, WindowInfo};
use crate::refs_test_support::HomeGuard;

struct NoopAdapter;
impl PlatformAdapter for NoopAdapter {}

struct WaitSnapshotAdapter;

impl PlatformAdapter for WaitSnapshotAdapter {
    fn list_windows(&self, _filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "FixtureApp".into(),
            pid: 1,
            bounds: None,
            is_focused: true,
        }])
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &crate::adapter::TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        Ok(AccessibilityNode {
            ref_id: None,
            role: "window".into(),
            name: Some("Doc".into()),
            value: None,
            description: None,
            hint: None,
            states: vec![],
            available_actions: vec![],
            bounds: None,
            children_count: None,
            children: vec![AccessibilityNode {
                ref_id: None,
                role: "button".into(),
                name: Some("Submit".into()),
                value: None,
                description: None,
                hint: None,
                states: vec![],
                available_actions: vec![],
                bounds: None,
                children_count: None,
                children: vec![],
            }],
        })
    }
}

fn base_args() -> SnapshotArgs {
    SnapshotArgs {
        app: None,
        window_id: None,
        max_depth: 8,
        include_bounds: false,
        interactive_only: false,
        compact: false,
        surface: SnapshotSurface::Window,
        skeleton: false,
        root_ref: None,
        snapshot_id: None,
    }
}

fn args_with_surface(surface: SnapshotSurface) -> SnapshotArgs {
    SnapshotArgs {
        surface,
        root_ref: Some("@e3".into()),
        ..base_args()
    }
}

#[test]
fn test_tree_options_clamps_skeleton_depth() {
    let mut args = base_args();
    args.skeleton = true;

    let opts = tree_options(&args);

    assert_eq!(opts.max_depth, 3);
    assert!(
        opts.skeleton,
        "skeleton flag must propagate for full snapshots"
    );
}

#[test]
fn test_tree_options_suppresses_skeleton_for_drill_down() {
    let mut args = base_args();
    args.skeleton = true;
    args.root_ref = Some("@e3".into());

    let opts = tree_options(&args);

    assert_eq!(
        opts.max_depth, 8,
        "depth must not be clamped for drill-down"
    );
    assert!(
        !opts.skeleton,
        "skeleton flag must be suppressed for drill-down so build_subtree \
         returns the full subtree and allocate_refs does not tag anchors"
    );
}

#[test]
fn test_root_with_menu_surface_rejected() {
    let args = args_with_surface(SnapshotSurface::Menu);
    let err = execute(args, &NoopAdapter, &CommandContext::default())
        .expect_err("should reject --root + --surface");
    match err {
        AppError::Adapter(adapter_err) => {
            assert_eq!(adapter_err.code, ErrorCode::InvalidArgs);
            assert!(
                adapter_err.message.contains("--root") && adapter_err.message.contains("--surface"),
                "error message should name both flags, got: {}",
                adapter_err.message
            );
        }
        other => panic!("expected Adapter(InvalidArgs), got {other:?}"),
    }
}

#[test]
fn test_root_with_window_surface_does_not_short_circuit_validation() {
    let args = args_with_surface(SnapshotSurface::Window);
    let result = execute(args, &NoopAdapter, &CommandContext::default());
    assert!(
        result.is_err(),
        "NoopAdapter cannot satisfy run_from_ref so this must error"
    );
    if let AppError::Adapter(adapter_err) = result.unwrap_err() {
        assert_ne!(
            adapter_err.code,
            ErrorCode::InvalidArgs,
            "Window surface must NOT trigger the --surface validation guard"
        );
    }
}

#[test]
fn test_invalid_root_ref_format_returns_invalid_args() {
    let args = SnapshotArgs {
        root_ref: Some("not-a-ref".into()),
        ..base_args()
    };
    let err = execute(args, &NoopAdapter, &CommandContext::default())
        .expect_err("malformed --root should fail");
    match err {
        AppError::Adapter(adapter_err) => {
            assert_eq!(
                adapter_err.code,
                ErrorCode::InvalidArgs,
                "malformed ref must return INVALID_ARGS, not STALE_REF"
            );
        }
        other => panic!("expected Adapter(InvalidArgs), got {other:?}"),
    }
}

#[test]
fn test_valid_root_ref_format_does_not_trigger_invalid_args() {
    let args = SnapshotArgs {
        root_ref: Some("@e42".into()),
        ..base_args()
    };
    let err = execute(args, &NoopAdapter, &CommandContext::default())
        .expect_err("NoopAdapter cannot resolve ref");
    if let AppError::Adapter(adapter_err) = err {
        assert_ne!(
            adapter_err.code,
            ErrorCode::InvalidArgs,
            "well-formed ref must not trigger INVALID_ARGS"
        );
    }
}

#[test]
fn wait_for_selector_returns_matched_snapshot() {
    let _guard = HomeGuard::new();
    let mut args = base_args();
    args.app = Some("FixtureApp".into());
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: "button:Submit".into(),
        gone: false,
        timeout_ms: 5_000,
    }));
    let value = execute(args, &WaitSnapshotAdapter, &context).unwrap();
    assert_eq!(value["matched_selector"], "button:Submit");
    assert!(value["snapshot_id"].as_str().is_some());
}

#[test]
fn root_and_wait_for_are_mutually_exclusive() {
    let mut args = base_args();
    args.root_ref = Some("@e1".into());
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: "button:Submit".into(),
        gone: false,
        timeout_ms: 5_000,
    }));
    let err = execute(args, &NoopAdapter, &context).expect_err("root + wait must fail");
    match err {
        AppError::Adapter(adapter_err) => {
            assert_eq!(adapter_err.code, ErrorCode::InvalidArgs);
        }
        other => panic!("expected InvalidArgs, got {other:?}"),
    }
}
