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

/// The gate is only sound if `IsPassword` arrives in the same batch as the
/// properties it gates; a separate read would cost the round trip KTD5 exists
/// to avoid and would open a window where the gate has no input.
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

    assert!(!TreeProperty::AutomationId.is_value_bearing());
    assert!(!TreeProperty::ClassName.is_value_bearing());
    assert!(!TreeProperty::BoundingRectangle.is_value_bearing());
    assert!(!TreeProperty::IsPassword.is_value_bearing());
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

/// A2-5 measured that UIA property ids are build-specific and named 2.2 as
/// the place a hand-written table would fail silently, so the source must
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
