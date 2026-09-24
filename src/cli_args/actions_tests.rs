use super::*;
use crate::cli_args::drag::DragCliArgs;
use clap::CommandFactory;

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
fn hover_help_discloses_stateless_duration_rejection() {
    let help = HoverArgs::command().render_long_help().to_string();
    assert!(help.contains("positive values are rejected in stateless mode"));
    assert!(help.contains("wait <ms>"));
}

#[test]
fn drag_cli_args_cli_omitted_timeout_defaults_to_5000() {
    let args = DragCliArgs::try_parse_from(["drag", "--from", "@e1", "--to", "@e2"]).unwrap();
    assert_eq!(args.timeout_ms, 5000);
    assert_eq!(args.target.from.as_deref(), Some("@e1"));
    assert_eq!(args.target.to.as_deref(), Some("@e2"));
}

#[test]
fn drag_cli_args_batch_json_omitted_timeout_defaults_to_5000() {
    let args: DragCliArgs =
        serde_json::from_value(serde_json::json!({ "from": "@e1", "to": "@e2" })).unwrap();
    assert_eq!(args.timeout_ms, 5000);
}

/// F10 regression: `MouseClickArgs`/`MousePointArgs` (mouse-click, mouse-down,
/// mouse-up) previously had no `--modifiers` flag at all — only mouse-wheel
/// carried one — so a chorded click was unreachable from the CLI regardless
/// of what the macOS layer supported. Each pair proves the repeatable flag
/// parses on the CLI and that legacy batch JSON without the key still
/// deserializes (serde default empty), matching `MouseWheelArgs`'s existing
/// contract.
#[test]
fn mouse_click_args_cli_modifiers_repeatable_flag_parses() {
    let args = MouseClickArgs::try_parse_from([
        "mouse-click",
        "--xy",
        "10,20",
        "--modifiers",
        "cmd",
        "--modifiers",
        "shift",
    ])
    .unwrap();
    assert_eq!(args.modifiers, vec!["cmd".to_string(), "shift".to_string()]);
}

#[test]
fn mouse_click_args_cli_omitted_modifiers_defaults_to_empty() {
    let args = MouseClickArgs::try_parse_from(["mouse-click", "--xy", "10,20"]).unwrap();
    assert!(args.modifiers.is_empty());
}

#[test]
fn mouse_click_args_batch_json_without_modifiers_key_still_deserializes() {
    let args: MouseClickArgs =
        serde_json::from_value(serde_json::json!({ "xy": "10,20" })).unwrap();
    assert!(args.modifiers.is_empty());
}

#[test]
fn background_pointer_flags_parse_on_hover_and_mouse_commands() {
    let hover = HoverArgs::try_parse_from(["hover", "@s1:e1", "--background"]).unwrap();
    let click = MouseClickArgs::try_parse_from([
        "mouse-click",
        "--background",
        "--window-id",
        "w-9555",
        "--xy",
        "10,20",
    ])
    .unwrap();
    let moved = MouseMoveArgs::try_parse_from([
        "mouse-move",
        "--background",
        "--window-id",
        "w-9555",
        "--xy",
        "10,20",
    ])
    .unwrap();

    assert!(hover.background && hover.window_id.is_none());
    assert!(click.background);
    assert_eq!(click.window_id.as_deref(), Some("w-9555"));
    assert!(moved.background);
    assert_eq!(moved.window_id.as_deref(), Some("w-9555"));
}

#[test]
fn background_pointer_flags_default_off_for_cli_and_batch() {
    let cli = MouseClickArgs::try_parse_from(["mouse-click", "--xy", "10,20"]).unwrap();
    let batch: HoverArgs = serde_json::from_value(serde_json::json!({ "ref_id": "@e1" })).unwrap();

    assert!(!cli.background && cli.window_id.is_none());
    assert!(!batch.background && batch.window_id.is_none());
}

#[test]
fn mouse_point_args_cli_modifiers_repeatable_flag_parses() {
    let args =
        MousePointArgs::try_parse_from(["mouse-down", "--xy", "10,20", "--modifiers", "ctrl"])
            .unwrap();
    assert_eq!(args.modifiers, vec!["ctrl".to_string()]);
}

#[test]
fn mouse_point_args_cli_omitted_modifiers_defaults_to_empty() {
    let args = MousePointArgs::try_parse_from(["mouse-up", "--xy", "10,20"]).unwrap();
    assert!(args.modifiers.is_empty());
}

#[test]
fn mouse_point_args_batch_json_without_modifiers_key_still_deserializes() {
    let args: MousePointArgs =
        serde_json::from_value(serde_json::json!({ "xy": "10,20" })).unwrap();
    assert!(args.modifiers.is_empty());
}

/// Displays left of or above the primary display have negative global
/// coordinates, so every coordinate flag must accept a value starting with `-`.
#[test]
fn coordinate_flags_accept_negative_coordinates() {
    let click = MouseClickArgs::try_parse_from([
        "mouse-click",
        "--background",
        "--window-id",
        "w-6015",
        "--xy",
        "-1580.5,966.5",
    ])
    .unwrap();
    assert_eq!(click.xy, "-1580.5,966.5");

    let moved = MouseMoveArgs::try_parse_from(["mouse-move", "--xy", "-10,-20"]).unwrap();
    assert_eq!(moved.xy, "-10,-20");

    let hover = HoverArgs::try_parse_from(["hover", "--xy", "-3762.5,1940"]).unwrap();
    assert_eq!(hover.xy.as_deref(), Some("-3762.5,1940"));

    let down = MousePointArgs::try_parse_from(["mouse-down", "--xy", "-1,2"]).unwrap();
    assert_eq!(down.xy, "-1,2");

    let drag = crate::cli_args::drag::DragCliArgs::try_parse_from([
        "drag",
        "--from-xy",
        "-5,6",
        "--to-xy",
        "-7,-8",
    ])
    .unwrap();
    assert_eq!(drag.target.from_xy.as_deref(), Some("-5,6"));
    assert_eq!(drag.target.to_xy.as_deref(), Some("-7,-8"));
}

#[test]
fn background_wheel_flags_parse_on_mouse_wheel_and_scroll() {
    use crate::cli_args::mouse_wheel::MouseWheelArgs;

    let wheel = MouseWheelArgs::try_parse_from([
        "mouse-wheel",
        "--background",
        "--window-id",
        "w-9555",
        "--x",
        "-1580.5",
        "--y",
        "-20",
        "--dy",
        "-5",
    ])
    .unwrap();
    assert!(wheel.background);
    assert_eq!(wheel.window_id.as_deref(), Some("w-9555"));
    assert_eq!((wheel.x, wheel.y, wheel.dy), (-1580.5, -20.0, -5.0));

    let scroll = ScrollArgs::try_parse_from(["scroll", "@s1:e1", "--background"]).unwrap();
    assert!(scroll.background);

    let batch: MouseWheelArgs =
        serde_json::from_value(serde_json::json!({ "x": 1.0, "y": 2.0 })).unwrap();
    assert!(!batch.background && batch.window_id.is_none());
    let batch: ScrollArgs = serde_json::from_value(serde_json::json!({ "ref_id": "@e1" })).unwrap();
    assert!(!batch.background);
}
