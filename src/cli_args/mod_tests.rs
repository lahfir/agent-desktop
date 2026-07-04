use super::*;
use agent_desktop_core::adapter::SnapshotSurface;
use clap::ValueEnum;

/// The exhaustive set of `SnapshotSurface` values the CLI's `Surface` enum is
/// meant to expose. `SnapshotSurface` is `#[non_exhaustive]`, so a new variant
/// added to it will not force `Surface::to_core`'s match arms (which match on
/// the local `Surface`, not on `SnapshotSurface`) to change — this list is the
/// independent tripwire that must be updated by hand alongside any new
/// `Surface`/`to_core` arm.
const EXPECTED_CORE_SURFACES: &[SnapshotSurface] = &[
    SnapshotSurface::Window,
    SnapshotSurface::Focused,
    SnapshotSurface::Menu,
    SnapshotSurface::Menubar,
    SnapshotSurface::Sheet,
    SnapshotSurface::Popover,
    SnapshotSurface::Alert,
];

/// Keeps the CLI `Surface` enum and the hand-maintained `EXPECTED_CORE_SURFACES`
/// tripwire in one-to-one correspondence: every `Surface` variant (enumerated
/// via its `ValueEnum` derive, so this side stays exhaustive automatically) maps
/// through `.to_core()` to a distinct `SnapshotSurface`, the tripwire lists each
/// of those exactly once, and the two sets match. An inconsistent hand-edit — a
/// `to_core` arm that collides with another, or the tripwire updated without a
/// matching `Surface`/`to_core` arm (or the reverse) — fails here. The one gap it
/// cannot close: because `SnapshotSurface` is `#[non_exhaustive]` with no
/// reflection, a new core variant added with no CLI-side edit keeps both sides
/// their current size and passes — the by-hand tripwire update is the only
/// checkpoint for that case (see `EXPECTED_CORE_SURFACES`).
#[test]
fn surface_to_core_maps_one_to_one_onto_expected_core_surfaces() {
    let cli_variants = Surface::value_variants();
    assert_eq!(
        cli_variants.len(),
        EXPECTED_CORE_SURFACES.len(),
        "Surface has {} variants but EXPECTED_CORE_SURFACES has {} — \
         update EXPECTED_CORE_SURFACES (and Surface::to_core) to match",
        cli_variants.len(),
        EXPECTED_CORE_SURFACES.len()
    );

    for (i, a) in EXPECTED_CORE_SURFACES.iter().enumerate() {
        for (j, b) in EXPECTED_CORE_SURFACES.iter().enumerate() {
            assert!(
                i == j || a != b,
                "EXPECTED_CORE_SURFACES lists SnapshotSurface::{a:?} more than once — \
                 each expected core surface must appear exactly once"
            );
        }
    }

    let mapped: Vec<SnapshotSurface> = cli_variants.iter().map(Surface::to_core).collect();
    for (i, a) in mapped.iter().enumerate() {
        for (j, b) in mapped.iter().enumerate() {
            assert!(
                i == j || a != b,
                "Surface::{:?} and Surface::{:?} both map to the same \
                 SnapshotSurface::{a:?} via to_core — mapping must be distinct",
                cli_variants[i],
                cli_variants[j]
            );
        }
    }

    for expected in EXPECTED_CORE_SURFACES {
        assert!(
            mapped.contains(expected),
            "no Surface variant maps to SnapshotSurface::{expected:?} via to_core — \
             a Surface arm is missing for this core surface"
        );
    }
}

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
