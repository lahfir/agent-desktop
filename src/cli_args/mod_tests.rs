use super::*;

/// God-object regression: `FindArgs` used to carry 14 flat fields; the
/// match-criteria and result-shaping groups now live in
/// `FindFilterArgs`/`FindSelectionArgs`, flattened back on. Proves the CLI
/// surface (flag names, `conflicts_with_all`) is unchanged by the
/// regrouping — a missing `#[command(flatten)]` would make clap reject these
/// flags as unrecognized.
#[test]
fn find_args_cli_flags_still_resolve_through_flattened_groups() {
    let args = FindArgs::try_parse_from([
        "find", "--role", "button", "--name", "Save", "--exact", "--first",
    ])
    .unwrap();

    assert_eq!(args.filter.role.as_deref(), Some("button"));
    assert_eq!(args.filter.name.as_deref(), Some("Save"));
    assert!(args.filter.exact);
    assert!(args.selection.first);
}

#[test]
fn find_args_selection_conflicts_still_enforced_across_the_flatten_boundary() {
    let err = FindArgs::try_parse_from(["find", "--first", "--last"]).unwrap_err();

    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

/// Batch JSON is decoded straight into `FindArgs` via serde; a flat payload
/// (the shape every existing caller sends) must still deserialize now that
/// the Rust type nests `filter`/`selection`.
#[test]
fn find_args_batch_json_flat_payload_deserializes_into_nested_groups() {
    let args: FindArgs = serde_json::from_value(serde_json::json!({
        "role": "checkbox",
        "native_id": "agree",
        "count": true
    }))
    .unwrap();

    assert_eq!(args.filter.role.as_deref(), Some("checkbox"));
    assert_eq!(args.filter.native_id.as_deref(), Some("agree"));
    assert!(args.selection.count);
    assert!(args.states.is_empty());
}

/// `WindowScope` (`app`/`window_id`) is shared across `FindArgs`,
/// `SnapshotArgs`, and `ScreenshotArgs` via `#[serde(flatten)]`, each also
/// carrying `#[serde(deny_unknown_fields)]` (`F6`). A flat batch payload
/// (the shape every existing caller sends) must still deserialize into
/// `.scope.app`/`.scope.window_id` now that the selector lives on its own
/// struct instead of directly on `FindArgs`.
#[test]
fn find_args_batch_json_flat_scope_deserializes_into_scope_group() {
    let args: FindArgs = serde_json::from_value(serde_json::json!({
        "app": "Finder",
        "window_id": "w-2"
    }))
    .unwrap();

    assert_eq!(args.scope.app.as_deref(), Some("Finder"));
    assert_eq!(args.scope.window_id.as_deref(), Some("w-2"));
    assert!(args.filter.role.is_none());
}

/// `WindowScope`'s `deny_unknown_fields` must still reject a typo'd scope
/// field on `FindArgs` instead of silently dropping the window filter — a
/// mistyped `app` (`ap`) or `window_id` (`windo_id`) should fail loudly
/// rather than deserialize into a scopeless, all-windows match.
#[test]
fn find_args_batch_json_rejects_scope_field_typo() {
    let err = serde_json::from_value::<FindArgs>(serde_json::json!({
        "ap": "Finder",
        "window_id": "w-2"
    }))
    .unwrap_err();
    assert!(err.to_string().contains("`ap`"));

    let err = serde_json::from_value::<FindArgs>(serde_json::json!({
        "app": "Finder",
        "windo_id": "w-2"
    }))
    .unwrap_err();
    assert!(err.to_string().contains("`windo_id`"));
}

/// Same `WindowScope` flatten, pinned for `SnapshotArgs` (`F6`): a flat
/// `app`/`window_id` payload must still populate `.scope.*` alongside
/// `SnapshotArgs`'s own defaulted fields.
#[test]
fn snapshot_args_batch_json_flat_scope_deserializes_into_scope_group() {
    let args: SnapshotArgs = serde_json::from_value(serde_json::json!({
        "app": "Finder",
        "window_id": "w-2"
    }))
    .unwrap();

    assert_eq!(args.scope.app.as_deref(), Some("Finder"));
    assert_eq!(args.scope.window_id.as_deref(), Some("w-2"));
    assert_eq!(args.max_depth, 10);
}

/// `SnapshotArgs` inherits `WindowScope`'s `deny_unknown_fields`; a typo'd
/// scope field must still be rejected rather than silently ignored.
#[test]
fn snapshot_args_batch_json_rejects_scope_field_typo() {
    let err = serde_json::from_value::<SnapshotArgs>(serde_json::json!({
        "ap": "Finder",
        "window_id": "w-2"
    }))
    .unwrap_err();
    assert!(err.to_string().contains("`ap`"));

    let err = serde_json::from_value::<SnapshotArgs>(serde_json::json!({
        "app": "Finder",
        "windo_id": "w-2"
    }))
    .unwrap_err();
    assert!(err.to_string().contains("`windo_id`"));
}

/// Same `WindowScope` flatten, pinned for `ScreenshotArgs` (`F6`): a flat
/// `app`/`window_id` payload must still populate `.scope.*` alongside
/// `ScreenshotArgs`'s own optional fields.
#[test]
fn screenshot_args_batch_json_flat_scope_deserializes_into_scope_group() {
    let args: ScreenshotArgs = serde_json::from_value(serde_json::json!({
        "app": "Finder",
        "window_id": "w-2"
    }))
    .unwrap();

    assert_eq!(args.scope.app.as_deref(), Some("Finder"));
    assert_eq!(args.scope.window_id.as_deref(), Some("w-2"));
    assert!(args.screen.is_none());
}

/// `ScreenshotArgs` inherits `WindowScope`'s `deny_unknown_fields`; a typo'd
/// scope field must still be rejected rather than silently ignored.
#[test]
fn screenshot_args_batch_json_rejects_scope_field_typo() {
    let err = serde_json::from_value::<ScreenshotArgs>(serde_json::json!({
        "ap": "Finder",
        "window_id": "w-2"
    }))
    .unwrap_err();
    assert!(err.to_string().contains("`ap`"));

    let err = serde_json::from_value::<ScreenshotArgs>(serde_json::json!({
        "app": "Finder",
        "windo_id": "w-2"
    }))
    .unwrap_err();
    assert!(err.to_string().contains("`windo_id`"));
}
