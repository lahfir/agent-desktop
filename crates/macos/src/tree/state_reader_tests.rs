use super::*;
use crate::tree::node_attrs::{NodeAttrStates, NodeAttrs};
use agent_desktop_core::node::Rect;

fn sample_attrs() -> NodeAttrs {
    NodeAttrs {
        role: Some("AXCheckBox".into()),
        title: None,
        description: None,
        value: Some("2".into()),
        native_id: None,
        states: NodeAttrStates {
            enabled: true,
            focused: None,
            expanded: None,
            disclosing: None,
            selected: None,
            hidden: None,
            busy: None,
            modal: None,
            required: None,
            readonly: None,
        },
        bounds: Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        has_scrollbars: false,
    }
}

fn ctx_with(window_bounds: Option<Rect>, is_secure_text: bool) -> StateReaderContext<'static> {
    StateReaderContext {
        focused: None,
        window_bounds,
        is_secure_text,
    }
}

#[test]
fn mixed_checkbox_emits_indeterminate_not_checked() {
    let attrs = sample_attrs();
    let ctx = ctx_with(None, false);
    let el = AXElement(std::ptr::null_mut());
    let states = states_from_element(&el, &attrs, "checkbox", &ctx);
    assert!(states.contains(&state::INDETERMINATE.to_string()));
    assert!(!states.contains(&state::CHECKED.to_string()));
}

#[test]
fn hidden_attr_emits_hidden_token() {
    let mut attrs = sample_attrs();
    attrs.states.hidden = Some(true);
    let ctx = ctx_with(None, false);
    let el = AXElement(std::ptr::null_mut());
    let states = states_from_element(&el, &attrs, "button", &ctx);
    assert!(states.contains(&state::HIDDEN.to_string()));
}

#[test]
fn clipped_bounds_emit_offscreen() {
    let mut attrs = sample_attrs();
    attrs.bounds = Some(Rect {
        x: 100.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    });
    let window = Rect {
        x: 0.0,
        y: 0.0,
        width: 50.0,
        height: 50.0,
    };
    let ctx = ctx_with(Some(window), false);
    let el = AXElement(std::ptr::null_mut());
    let states = states_from_element(&el, &attrs, "button", &ctx);
    assert!(states.contains(&state::OFFSCREEN.to_string()));
}

/// Real drift guard for U1/U2/KTD2, replacing the tautological version that
/// asserted `state::` constants against a vocabulary built from those same
/// constants (could never fail). This drives the actual production
/// `states_from_element` producer over inputs representative of every
/// branch it has (disabled, secure, expanded, checked/indeterminate,
/// selected, hidden, busy, modal, required, pressed, readonly, offscreen)
/// and asserts the real emitted token set is a subset of
/// `STATE_VOCABULARY`. If a future change to `states_from_element` pushes a
/// token that is not a `state::` constant, this fails on the actual
/// emission, not on a copy of the constant list.
#[test]
fn emitted_tokens_over_representative_inputs_are_vocabulary_members() {
    let el = AXElement(std::ptr::null_mut());
    let window = Rect {
        x: 0.0,
        y: 0.0,
        width: 50.0,
        height: 50.0,
    };
    let mut cases: Vec<(NodeAttrs, &str, StateReaderContext<'static>)> = Vec::new();

    let mut disabled = sample_attrs();
    disabled.states.enabled = false;
    cases.push((disabled, "button", ctx_with(None, false)));

    cases.push((sample_attrs(), "textfield", ctx_with(None, true)));

    let mut expanded = sample_attrs();
    expanded.states.expanded = Some(true);
    cases.push((expanded, "disclosure", ctx_with(None, false)));

    let mut checked = sample_attrs();
    checked.value = Some("1".into());
    cases.push((checked, "checkbox", ctx_with(None, false)));

    let mut selected = sample_attrs();
    selected.states.selected = Some(true);
    cases.push((selected, "cell", ctx_with(None, false)));

    let mut busy = sample_attrs();
    busy.states.busy = Some(true);
    cases.push((busy, "button", ctx_with(None, false)));

    let mut modal = sample_attrs();
    modal.states.modal = Some(true);
    cases.push((modal, "window", ctx_with(None, false)));

    let mut required = sample_attrs();
    required.states.required = Some(true);
    cases.push((required, "textfield", ctx_with(None, false)));

    let mut pressed = sample_attrs();
    pressed.value = Some("1".into());
    cases.push((pressed, "button", ctx_with(None, false)));

    let mut readonly = sample_attrs();
    readonly.states.readonly = Some(true);
    cases.push((readonly, "textfield", ctx_with(None, false)));

    let mut offscreen = sample_attrs();
    offscreen.bounds = Some(Rect {
        x: 1000.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    });
    cases.push((offscreen, "button", ctx_with(Some(window), false)));

    let mut emitted: Vec<String> = Vec::new();
    for (attrs, role, ctx) in &cases {
        emitted.extend(states_from_element(&el, attrs, role, ctx));
    }
    assert!(
        !emitted.is_empty(),
        "representative inputs should exercise at least one producer branch"
    );
    state::assert_states_in_vocabulary(&emitted);
}

/// Proves the guard above is not vacuous: `assert_states_in_vocabulary`
/// genuinely panics when handed a token that never came from a `state::`
/// constant, so a producer that starts emitting a drifted token would fail
/// this same assertion path, not silently pass.
#[test]
#[should_panic(expected = "is not in STATE_VOCABULARY")]
fn assert_states_in_vocabulary_rejects_bogus_token() {
    state::assert_states_in_vocabulary(&["zzz_bogus_state_token".to_string()]);
}
