/// Compares text exactly and numeric controls with an absolute readback tolerance.
pub fn value_matches(role: &str, expected: &str, observed: Option<&str>) -> bool {
    let Some(observed) = observed else {
        return false;
    };
    if expected == observed {
        return true;
    }
    if !matches!(role, "slider" | "incrementor" | "scrollbar" | "handle") {
        return false;
    }
    if let (Ok(expected), Ok(observed)) = (expected.parse::<i128>(), observed.parse::<i128>()) {
        return expected == observed;
    }
    match (expected.parse::<f64>(), observed.parse::<f64>()) {
        (Ok(expected), Ok(observed)) => {
            expected.is_finite()
                && observed.is_finite()
                && expected.abs().max(observed.abs()) < 9_007_199_254_740_992.0
                && (expected - observed).abs() <= 1e-6
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::value_matches;

    #[test]
    fn numeric_readback_tolerance_does_not_hide_different_text_or_integer_targets() {
        for (role, expected, observed, matches) in [
            ("textfield", "001", "1", false),
            ("slider", "50", "50.00", true),
            ("slider", "50", "50.0000004", true),
            ("incrementor", "1000000000", "1000000001", false),
            ("incrementor", "1000000000", "1000000001.0", false),
            ("incrementor", "9007199254740993", "9007199254740992", false),
            (
                "incrementor",
                "9007199254740993",
                "9007199254740992.0",
                false,
            ),
            ("slider", "1", "inf", false),
        ] {
            assert_eq!(value_matches(role, expected, Some(observed)), matches);
        }
        assert!(!value_matches("slider", "1", None));
    }
}
