use super::*;

/// Code review found the `IsPassword` divergence `withholds_content` guards
/// against - a provider answering a documented-`VT_BOOL` property as
/// `VT_I4(1)` - reaching every other gate too, because `gate_open` read only
/// `flag()`. Both halves of the fix are pinned in one test: a nonzero
/// `Known` number opens the gate `withholds_content` already accepts, and a
/// gate nothing read still does not open - without the second assertion a
/// fix that opened every unread gate unconditionally would also pass.
#[test]
fn a_nonzero_known_number_opens_the_gate_but_an_unread_gate_still_does_not() {
    let opened_by_number = ElementProperties::from_reads(vec![
        (
            TreeProperty::ToggleAvailable,
            PropertyOutcome::Known(PropertyValue::Number(1)),
        ),
        (
            TreeProperty::ToggleState,
            PropertyOutcome::Known(PropertyValue::Number(1)),
        ),
    ]);
    assert_eq!(
        opened_by_number.gated_number(TreeProperty::ToggleState),
        Some(1),
        "a provider answering IsTogglePatternAvailable as VT_I4(1) must open the gate, \
         the same way withholds_content already accepts a nonzero VT_I4 IsPassword"
    );

    let gate_never_read = ElementProperties::from_reads(vec![(
        TreeProperty::ToggleState,
        PropertyOutcome::Known(PropertyValue::Number(1)),
    )]);
    assert_eq!(
        gate_never_read.gated_number(TreeProperty::ToggleState),
        None,
        "a gate nothing read must stay closed - reading an unread gate as open \
         would be a worse bug than the one being fixed"
    );
}

/// The same divergence reaches an `…Available` property read directly
/// through `is_true`, not only a state property read through its gate: the
/// `…Available` properties are themselves ungated, so `gated_flag`'s own
/// terminal read - not `gate_open` - is what a caller like
/// `resolve_actions`/`resolve_role` depends on here.
#[test]
fn is_true_reads_a_nonzero_known_number_on_an_ungated_available_property() {
    let properties = ElementProperties::from_reads(vec![(
        TreeProperty::ToggleAvailable,
        PropertyOutcome::Known(PropertyValue::Number(1)),
    )]);

    assert!(
        properties.is_true(TreeProperty::ToggleAvailable),
        "IsTogglePatternAvailable answered as VT_I4(1) must be true, the same way \
         withholds_content already accepts a nonzero VT_I4 IsPassword"
    );
}
