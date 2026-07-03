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
