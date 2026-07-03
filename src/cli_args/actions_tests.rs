use super::*;

/// F2 regression: `TypeArgs`, `SetValueArgs`, `SelectArgs`, `ScrollArgs`,
/// `HoverArgs`, and `DragCliArgs` previously had no `--timeout-ms` field at
/// all, so the auto-wait budget these commands' CLI docs promise (R7) was
/// unreachable for them. Each pair below proves the paired clap/serde default
/// (5000) actually fires from both entry points: clap's `default_value_t`
/// covers the CLI, and `#[serde(default = "default_ref_timeout_ms")]` covers
/// batch JSON, since clap defaults never fire during `serde_json::from_value`.
#[test]
fn type_args_cli_omitted_timeout_defaults_to_5000() {
    let args = TypeArgs::try_parse_from(["type", "@e1", "hello"]).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn type_args_cli_explicit_timeout_is_honored() {
    let args = TypeArgs::try_parse_from(["type", "@e1", "hello", "--timeout-ms", "1500"]).unwrap();
    assert_eq!(args.timeout_ms, 1500);
}

#[test]
fn type_args_batch_json_omitted_timeout_defaults_to_5000() {
    let args: TypeArgs =
        serde_json::from_value(serde_json::json!({ "ref_id": "@e1", "text": "hello" })).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn set_value_args_cli_omitted_timeout_defaults_to_5000() {
    let args = SetValueArgs::try_parse_from(["set-value", "@e1", "value"]).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn set_value_args_batch_json_omitted_timeout_defaults_to_5000() {
    let args: SetValueArgs =
        serde_json::from_value(serde_json::json!({ "ref_id": "@e1", "value": "v" })).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn select_args_cli_omitted_timeout_defaults_to_5000() {
    let args = SelectArgs::try_parse_from(["select", "@e1", "choice"]).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn select_args_batch_json_omitted_timeout_defaults_to_5000() {
    let args: SelectArgs =
        serde_json::from_value(serde_json::json!({ "ref_id": "@e1", "value": "choice" })).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn scroll_args_cli_omitted_timeout_defaults_to_5000() {
    let args = ScrollArgs::try_parse_from(["scroll", "@e1"]).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn scroll_args_batch_json_omitted_timeout_defaults_to_5000() {
    let args: ScrollArgs = serde_json::from_value(serde_json::json!({ "ref_id": "@e1" })).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn hover_args_cli_omitted_timeout_defaults_to_5000() {
    let args = HoverArgs::try_parse_from(["hover", "@e1"]).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn hover_args_batch_json_omitted_timeout_defaults_to_5000() {
    let args: HoverArgs = serde_json::from_value(serde_json::json!({ "ref_id": "@e1" })).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn drag_cli_args_cli_omitted_timeout_defaults_to_5000() {
    let args = DragCliArgs::try_parse_from(["drag", "--from", "@e1", "--to", "@e2"]).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

#[test]
fn drag_cli_args_batch_json_omitted_timeout_defaults_to_5000() {
    let args: DragCliArgs =
        serde_json::from_value(serde_json::json!({ "from": "@e1", "to": "@e2" })).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}
