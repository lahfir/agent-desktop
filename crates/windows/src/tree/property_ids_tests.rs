use super::*;

#[test]
fn every_value_bearing_property_is_in_the_walk_set() {
    for property in TreeProperty::VALUE_BEARING {
        assert!(
            TreeProperty::WALK_SET.contains(&property),
            "{} is gated but never read, so the gate would never fire",
            property.as_str()
        );
    }
}

/// A15-7 measured every pattern-state property returning a default-looking
/// value on an element lacking the pattern, so a gate that is not itself in
/// the batch is a gate that cannot fire.
#[test]
fn every_gate_is_read_in_the_same_batch_as_the_property_it_gates() {
    for property in TreeProperty::WALK_SET {
        let Some(gate) = property.gate() else {
            continue;
        };
        assert!(
            TreeProperty::WALK_SET.contains(&gate),
            "{} is gated on {}, which the walk never reads",
            property.as_str(),
            gate.as_str()
        );
    }
}

/// The gate exists because of a measurement, so the set it covers is asserted
/// rather than left to whoever adds the next state arm.
#[test]
fn every_pattern_state_property_carries_a_gate() {
    for property in [
        TreeProperty::ToggleState,
        TreeProperty::ExpandCollapseState,
        TreeProperty::SelectionItemIsSelected,
        TreeProperty::ValueIsReadOnly,
        TreeProperty::SelectionCanSelectMultiple,
        TreeProperty::WindowIsModal,
    ] {
        assert!(
            property.gate().is_some(),
            "{} would be read ungated, which A15-7 measured as a false state on every inert node",
            property.as_str()
        );
    }
    assert!(TreeProperty::Name.gate().is_none());
    assert!(TreeProperty::IsEnabled.gate().is_none());
}

/// An element-valued property cannot travel through the tri-state classifier,
/// which decodes scalars and would report a successful read as a failed one.
#[test]
fn the_only_element_valued_property_is_the_label_relation() {
    assert!(TreeProperty::LabeledBy.is_element_valued());
    for property in TreeProperty::WALK_SET {
        assert_eq!(
            property.is_element_valued(),
            property == TreeProperty::LabeledBy,
            "{} is classified as element-valued",
            property.as_str()
        );
    }
}

/// The gate is only sound if `IsPassword` arrives in the same batch as the
/// properties it gates; a separate read would cost a round trip the batched
/// design avoids and would open a window where the gate has no input.
#[test]
fn the_walk_set_carries_the_flag_that_gates_it() {
    assert!(TreeProperty::WALK_SET.contains(&TreeProperty::IsPassword));
}

#[test]
fn the_gate_covers_exactly_the_properties_whose_content_comes_from_the_target() {
    assert!(TreeProperty::Name.is_value_bearing());
    assert!(TreeProperty::Value.is_value_bearing());
    assert!(TreeProperty::HelpText.is_value_bearing());
    assert!(TreeProperty::LegacyValue.is_value_bearing());
    assert!(TreeProperty::FullDescription.is_value_bearing());
    assert!(TreeProperty::LegacyDefaultAction.is_value_bearing());

    assert!(!TreeProperty::AutomationId.is_value_bearing());
    assert!(!TreeProperty::ClassName.is_value_bearing());
    assert!(!TreeProperty::BoundingRectangle.is_value_bearing());
    assert!(!TreeProperty::IsPassword.is_value_bearing());
}

/// Every property whose value is text read out of the target is gated,
/// checked against `carries_target_text()` - the property's own declaration -
/// rather than against a second hand-maintained list. A hand list that
/// happened to duplicate `VALUE_BEARING` let a text property added to
/// `WALK_SET` without joining either list escape both this test and the
/// secure-field gate, invisible until a password reached a trace export.
/// `carries_target_text()` is an exhaustive match with no catch-all arm, so a
/// new variant forces a compile-time decision instead of a silent `false`.
#[test]
fn no_text_property_reaches_the_walk_without_joining_the_gate() {
    for property in TreeProperty::WALK_SET {
        if !property.carries_target_text() {
            continue;
        }
        assert!(
            property.is_value_bearing(),
            "{} carries target-authored text into evidence without the secure gate",
            property.as_str()
        );
    }
}

/// `carries_target_text()` and `is_value_bearing()` are two independent
/// declarations of the same fact - one on the property itself, one in
/// `VALUE_BEARING` - and a property that carries target text must be
/// value-bearing regardless of whether the walk currently reads it, so this
/// checks the whole enum rather than only `WALK_SET`.
#[test]
fn carries_target_text_implies_value_bearing_across_every_property() {
    for property in [
        TreeProperty::Name,
        TreeProperty::AutomationId,
        TreeProperty::ClassName,
        TreeProperty::HelpText,
        TreeProperty::FullDescription,
        TreeProperty::LabeledBy,
        TreeProperty::Value,
        TreeProperty::LegacyValue,
        TreeProperty::BoundingRectangle,
        TreeProperty::IsPassword,
        TreeProperty::IsOffscreen,
        TreeProperty::IsEnabled,
        TreeProperty::IsControlElement,
        TreeProperty::IsContentElement,
        TreeProperty::IsKeyboardFocusable,
        TreeProperty::HasKeyboardFocus,
        TreeProperty::IsRequiredForForm,
        TreeProperty::IsDataValidForForm,
        TreeProperty::IsDialog,
        TreeProperty::ToggleState,
        TreeProperty::ExpandCollapseState,
        TreeProperty::SelectionItemIsSelected,
        TreeProperty::ValueIsReadOnly,
        TreeProperty::SelectionCanSelectMultiple,
        TreeProperty::WindowIsModal,
        TreeProperty::LegacyState,
        TreeProperty::LegacyDefaultAction,
        TreeProperty::InvokeAvailable,
        TreeProperty::ToggleAvailable,
        TreeProperty::ExpandCollapseAvailable,
        TreeProperty::SelectionItemAvailable,
        TreeProperty::SelectionAvailable,
        TreeProperty::ValueAvailable,
        TreeProperty::RangeValueAvailable,
        TreeProperty::ScrollAvailable,
        TreeProperty::ScrollItemAvailable,
        TreeProperty::WindowAvailable,
        TreeProperty::GridItemAvailable,
        TreeProperty::TableItemAvailable,
        TreeProperty::LegacyAvailable,
        TreeProperty::ProviderDescription,
        TreeProperty::ControlType,
        TreeProperty::RuntimeId,
    ] {
        assert!(
            !property.carries_target_text() || property.is_value_bearing(),
            "{} carries target text by its own declaration but is missing from VALUE_BEARING",
            property.as_str()
        );
    }
}

#[test]
fn the_walk_set_has_no_duplicate_entries() {
    let mut seen = Vec::new();
    for property in TreeProperty::WALK_SET {
        assert!(
            !seen.contains(&property),
            "{} appears twice",
            property.as_str()
        );
        seen.push(property);
    }
}

#[test]
fn every_property_names_itself_for_a_structured_error() {
    for property in TreeProperty::WALK_SET {
        assert!(!property.as_str().is_empty());
    }
    assert_eq!(TreeProperty::LegacyValue.as_str(), "LegacyIAccessibleValue");
}

#[cfg(target_os = "windows")]
#[test]
fn every_property_resolves_through_the_crate_generated_constants() {
    for property in TreeProperty::WALK_SET {
        let _ = uia_property(property);
    }
    let _ = uia_property(TreeProperty::ProviderDescription);
    let _ = uia_property(TreeProperty::ControlType);
    let _ = uia_property(TreeProperty::RuntimeId);
}

/// A2-5 measured that UIA property ids are build-specific and named this
/// module as the place a hand-written table would fail silently, so the source must
/// contain no bare property-id integer.
///
/// Matched as a whole token in the range UIA actually uses, not as the
/// substring "300": a prose mention of 300 milliseconds, or a `30_000` ms
/// constant, is not a property id and must not fail this.
#[test]
fn no_property_id_integer_appears_in_this_module() {
    for (name, source) in [
        ("property_ids.rs", include_str!("property_ids.rs")),
        ("properties.rs", include_str!("properties.rs")),
        ("cache.rs", include_str!("cache.rs")),
        (
            "element_properties.rs",
            include_str!("element_properties.rs"),
        ),
        ("roles.rs", include_str!("roles.rs")),
        ("actions.rs", include_str!("actions.rs")),
        ("states.rs", include_str!("states.rs")),
        ("name_evidence.rs", include_str!("name_evidence.rs")),
        ("property_outcome.rs", include_str!("property_outcome.rs")),
        ("walker.rs", include_str!("walker.rs")),
        ("walker_source.rs", include_str!("walker_source.rs")),
    ] {
        for (number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("///") || trimmed.starts_with("//!") {
                continue;
            }
            assert!(
                !contains_property_id_literal(line),
                "{name}:{} carries a UIA property-id literal: {line}",
                number + 1
            );
        }
    }
}

/// Reports whether a line contains a bare integer in UIA's property-id range.
///
/// A token is a candidate only when it is a whole number, five digits long,
/// and between 30000 and 30999 - the block UIA allocates property ids from.
fn contains_property_id_literal(line: &str) -> bool {
    line.split(|character: char| !(character.is_ascii_digit() || character == '_'))
        .filter(|token| !token.is_empty())
        .map(|token| token.replace('_', ""))
        .any(|token| {
            token.len() == 5
                && token
                    .parse::<u32>()
                    .is_ok_and(|value| (30_000..=30_999).contains(&value))
        })
}
