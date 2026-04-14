use crate::{
    adapter::{PlatformAdapter, SnapshotSurface},
    error::AppError,
    snapshot, snapshot_ref,
};
use serde_json::{json, Value};

pub struct SnapshotArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: SnapshotSurface,
    pub skeleton: bool,
    pub root_ref: Option<String>,
}

fn tree_options(args: &SnapshotArgs) -> crate::adapter::TreeOptions {
    let effective_depth = if args.skeleton && args.root_ref.is_none() {
        args.max_depth.min(3)
    } else {
        args.max_depth
    };

    crate::adapter::TreeOptions {
        max_depth: effective_depth,
        include_bounds: args.include_bounds,
        interactive_only: args.interactive_only,
        compact: args.compact,
        surface: args.surface,
        skeleton: args.skeleton,
    }
}

pub fn execute(args: SnapshotArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    tracing::debug!(
        "tree: snapshot app={:?} window_id={:?} max_depth={} interactive_only={} compact={}",
        args.app.as_deref().unwrap_or("(focused)"),
        args.window_id.as_deref().unwrap_or("(auto)"),
        args.max_depth,
        args.interactive_only,
        args.compact
    );

    let opts = tree_options(&args);

    if let Some(ref root) = args.root_ref {
        if !matches!(args.surface, SnapshotSurface::Window) {
            return Err(AppError::invalid_input(
                "--root cannot be combined with --surface",
            ));
        }
        return format_result(snapshot_ref::run_from_ref(adapter, &opts, root)?);
    }

    let result = snapshot::run(
        adapter,
        &opts,
        args.app.as_deref(),
        args.window_id.as_deref(),
    )?;

    format_result(result)
}

fn format_result(result: snapshot::SnapshotResult) -> Result<Value, AppError> {
    let ref_count = result.refmap.len();
    let tree = serde_json::to_value(&result.tree)?;
    let win = &result.window;

    tracing::debug!(
        "tree: snapshot complete app={:?} window={:?} refs={}",
        win.app,
        win.title,
        ref_count
    );

    Ok(json!({
        "app": win.app,
        "window": {
            "id": win.id,
            "title": win.title
        },
        "ref_count": ref_count,
        "tree": tree
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    struct NoopAdapter;
    impl PlatformAdapter for NoopAdapter {}

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
    }

    #[test]
    fn test_tree_options_preserves_depth_for_drill_down() {
        let mut args = base_args();
        args.skeleton = true;
        args.root_ref = Some("@e3".into());

        let opts = tree_options(&args);

        assert_eq!(opts.max_depth, 8);
    }

    #[test]
    fn test_root_with_menu_surface_rejected() {
        let args = args_with_surface(SnapshotSurface::Menu);
        let err = execute(args, &NoopAdapter).expect_err("should reject --root + --surface");
        match err {
            AppError::Adapter(adapter_err) => {
                assert_eq!(adapter_err.code, ErrorCode::InvalidArgs);
                assert!(
                    adapter_err.message.contains("--root")
                        && adapter_err.message.contains("--surface"),
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
        let result = execute(args, &NoopAdapter);
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
}
