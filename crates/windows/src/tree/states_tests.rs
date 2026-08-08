use agent_desktop_core::LocatorField;
use agent_desktop_core::state;

use crate::tree::properties::{ElementProperties, PropertyOutcome, PropertyValue};
use crate::tree::property_ids::TreeProperty;

use super::resolve_states;

fn flag(property: TreeProperty, value: bool) -> (TreeProperty, PropertyOutcome) {
    (property, PropertyOutcome::Known(PropertyValue::Flag(value)))
}

fn number(property: TreeProperty, value: i32) -> (TreeProperty, PropertyOutcome) {
    (
        property,
        PropertyOutcome::Known(PropertyValue::Number(value)),
    )
}

fn enabled_true() -> (TreeProperty, PropertyOutcome) {
    flag(TreeProperty::IsEnabled, true)
}

fn props(reads: Vec<(TreeProperty, PropertyOutcome)>) -> ElementProperties {
    ElementProperties::from_reads(reads)
}

fn known(field: LocatorField<Vec<String>>) -> Vec<String> {
    match field {
        LocatorField::Known(states) => states,
        other => panic!("expected LocatorField::Known, got {other:?}"),
    }
}

fn role_of(role: &str) -> LocatorField<String> {
    LocatorField::Known(role.to_string())
}

fn resolved(reads: Vec<(TreeProperty, PropertyOutcome)>, role: &str) -> Vec<String> {
    let mut all_reads = vec![enabled_true()];
    all_reads.extend(reads);
    known(resolve_states(&props(all_reads), &role_of(role)))
}

/// Every representative source this producer can fire, paired with the role
/// each needs to fire on. `enabled_true` is prepended by [`resolved`] so a
/// case here never accidentally hits the read-health fallback.
fn representative_cases() -> Vec<(Vec<(TreeProperty, PropertyOutcome)>, &'static str)> {
    vec![
        (vec![flag(TreeProperty::IsEnabled, false)], "button"),
        (vec![flag(TreeProperty::IsPassword, true)], "textfield"),
        (vec![flag(TreeProperty::IsOffscreen, true)], "button"),
        (
            vec![flag(TreeProperty::HasKeyboardFocus, true)],
            "textfield",
        ),
        (
            vec![flag(TreeProperty::IsRequiredForForm, true)],
            "textfield",
        ),
        (
            vec![flag(TreeProperty::IsRequiredForForm, true)],
            "textfield",
        ),
        (
            vec![
                flag(TreeProperty::ToggleAvailable, true),
                number(TreeProperty::ToggleState, 1),
            ],
            "checkbox",
        ),
        (
            vec![
                flag(TreeProperty::ToggleAvailable, true),
                number(TreeProperty::ToggleState, 2),
            ],
            "checkbox",
        ),
        (
            vec![
                flag(TreeProperty::ExpandCollapseAvailable, true),
                number(TreeProperty::ExpandCollapseState, 1),
            ],
            "treeitem",
        ),
        (
            vec![
                flag(TreeProperty::SelectionItemAvailable, true),
                flag(TreeProperty::SelectionItemIsSelected, true),
            ],
            "cell",
        ),
        (
            vec![
                flag(TreeProperty::ValueAvailable, true),
                flag(TreeProperty::ValueIsReadOnly, true),
            ],
            "textfield",
        ),
        (
            vec![
                flag(TreeProperty::SelectionAvailable, true),
                flag(TreeProperty::SelectionCanSelectMultiple, true),
            ],
            "listbox",
        ),
        (
            vec![
                flag(TreeProperty::WindowAvailable, true),
                flag(TreeProperty::WindowIsModal, true),
            ],
            "window",
        ),
        (
            vec![number(TreeProperty::LegacyState, 0x4000_0000)],
            "menuitem",
        ),
        (
            vec![number(TreeProperty::LegacyState, 0x0000_0800)],
            "button",
        ),
    ]
}

#[test]
fn emitted_tokens_over_representative_inputs_are_vocabulary_members() {
    let mut emitted = Vec::new();
    for (reads, role) in representative_cases() {
        emitted.extend(resolved(reads, role));
    }

    assert!(
        !emitted.is_empty(),
        "representative inputs should exercise at least one producer branch"
    );
    state::assert_states_in_vocabulary(&emitted);
}

#[test]
#[should_panic(expected = "is not in STATE_VOCABULARY")]
fn assert_states_in_vocabulary_rejects_bogus_token() {
    state::assert_states_in_vocabulary(&["zzz_bogus_state_token".to_string()]);
}

/// The role is `checkbox` on purpose. A non-toggleable role is suppressed by
/// the role gate whatever the availability gate does, so a static text here
/// would make this pass for a reason that has nothing to do with A15-7 - and
/// it would keep passing with the availability gate deleted.
#[test]
fn a15_7_toggle_state_without_toggle_available_emits_no_indeterminate() {
    let states = resolved(
        vec![
            flag(TreeProperty::ToggleAvailable, false),
            number(TreeProperty::ToggleState, 2),
        ],
        "checkbox",
    );

    assert!(!states.contains(&state::INDETERMINATE.to_string()));
    assert!(!states.contains(&state::CHECKED.to_string()));

    let advertised = resolved(
        vec![
            flag(TreeProperty::ToggleAvailable, true),
            number(TreeProperty::ToggleState, 2),
        ],
        "checkbox",
    );

    assert!(
        advertised.contains(&state::INDETERMINATE.to_string()),
        "the same read set with the pattern advertised must produce the token, or the case above proves nothing"
    );
}

#[test]
fn a15_7_readonly_without_value_available_emits_no_readonly() {
    let states = resolved(
        vec![
            flag(TreeProperty::ValueAvailable, false),
            flag(TreeProperty::ValueIsReadOnly, true),
        ],
        "statictext",
    );

    assert!(!states.contains(&state::READONLY.to_string()));
}

#[test]
fn leaf_node_expand_collapse_state_emits_no_expanded_token() {
    let states = resolved(
        vec![
            flag(TreeProperty::ExpandCollapseAvailable, true),
            number(TreeProperty::ExpandCollapseState, 3),
        ],
        "treeitem",
    );

    assert!(!states.contains(&state::EXPANDED.to_string()));
}

#[test]
fn toggle_state_on_non_toggleable_role_emits_no_checked() {
    let states = resolved(
        vec![
            flag(TreeProperty::ToggleAvailable, true),
            number(TreeProperty::ToggleState, 1),
        ],
        "statictext",
    );

    assert!(!states.contains(&state::CHECKED.to_string()));
    assert!(!states.contains(&state::PRESSED.to_string()));
}

/// `pressed` has no reachable source: `push_toggle_state`'s dead arm for
/// `role == "button"` was removed because `roles.rs` reclassifies any
/// toggle-available `Button` to `switch` before states resolve, so a
/// `button` role reaching this producer has never advertised
/// `ToggleAvailable` and never carries a `ToggleState` to read. See
/// `resolve_states`'s doc comment for the full reasoning. This is the
/// regression test for that guarantee across both toggle values the pattern
/// defines. Its input is deliberately unreachable in production; see
/// [`toggle_state_on_a_switch_role_emits_checked_and_indeterminate`] for the
/// reachable shape - do not "fix" this test by feeding it a real role.
#[test]
fn pressed_is_unproduced_for_a_button_role_at_any_toggle_state() {
    for toggle in [1, 2] {
        let states = resolved(
            vec![
                flag(TreeProperty::ToggleAvailable, true),
                number(TreeProperty::ToggleState, toggle),
            ],
            "button",
        );

        assert!(!states.contains(&state::PRESSED.to_string()));
        assert!(!states.contains(&state::CHECKED.to_string()));
    }
}

/// The reachable counterpart to the test above: `switch` is a role
/// `roles.rs` actually produces for a toggle-available control, and
/// `is_toggleable_role` confirms it is gated in before the assertion runs.
#[test]
fn toggle_state_on_a_switch_role_emits_checked_and_indeterminate() {
    assert!(agent_desktop_core::roles::is_toggleable_role("switch"));

    let checked = resolved(
        vec![
            flag(TreeProperty::ToggleAvailable, true),
            number(TreeProperty::ToggleState, 1),
        ],
        "switch",
    );
    assert!(checked.contains(&state::CHECKED.to_string()));

    let indeterminate = resolved(
        vec![
            flag(TreeProperty::ToggleAvailable, true),
            number(TreeProperty::ToggleState, 2),
        ],
        "switch",
    );
    assert!(indeterminate.contains(&state::INDETERMINATE.to_string()));
}

/// The regression test for the defect the dogfood run found, and the reason
/// `invalid` is unproduced.
///
/// `IsDataValidForForm` reads `false` on essentially everything, because
/// `false` is its default rather than a claim. Emitting `invalid` from it put
/// the token on **every node of every target measured** - 26 of 26 on Notepad,
/// 113 of 113 on Explorer - including static text, title bars and windows.
///
/// The contrast with `required` is the whole point: that arm reads the same
/// kind of form flag and is safe only because it emits on `true`.
#[test]
fn a_default_false_form_validity_flag_produces_no_invalid_token() {
    for role in ["statictext", "window", "textfield", "menuitem"] {
        let states = resolved(
            vec![
                flag(TreeProperty::IsDataValidForForm, false),
                enabled_true(),
            ],
            role,
        );

        assert!(
            !states.contains(&state::INVALID.to_string()),
            "a {role} was reported invalid because a form flag defaulted to false"
        );
    }

    assert!(
        resolved(
            vec![flag(TreeProperty::IsRequiredForForm, true), enabled_true()],
            "textfield",
        )
        .contains(&state::REQUIRED.to_string()),
        "the sibling arm that emits on a positive claim must still work, or this proves nothing"
    );
}

/// `invalid` has no usable Windows source and must not appear from any input.
#[test]
fn invalid_is_unproduced_whatever_the_read_set_says() {
    for value in [true, false] {
        for role in ["textfield", "checkbox", "statictext"] {
            let states = resolved(
                vec![
                    flag(TreeProperty::IsDataValidForForm, value),
                    enabled_true(),
                ],
                role,
            );
            assert!(!states.contains(&state::INVALID.to_string()));
        }
    }
}

#[test]
fn a_failed_state_read_yields_unknown_not_an_empty_known_list() {
    let properties = props(Vec::new());

    let result = resolve_states(&properties, &LocatorField::Unknown);

    assert!(matches!(result, LocatorField::Unknown));
}

#[test]
fn legacy_state_unrelated_bit_emits_neither_haspopup_nor_busy() {
    let states = resolved(
        vec![number(TreeProperty::LegacyState, 0x0010_0000)],
        "menuitem",
    );

    assert!(!states.contains(&state::HASPOPUP.to_string()));
    assert!(!states.contains(&state::BUSY.to_string()));
}

#[test]
fn legacy_state_haspopup_bit_alongside_an_unrelated_bit_emits_haspopup() {
    let states = resolved(
        vec![number(TreeProperty::LegacyState, 0x4010_0000)],
        "menuitem",
    );

    assert!(states.contains(&state::HASPOPUP.to_string()));
    assert!(!states.contains(&state::BUSY.to_string()));
}
