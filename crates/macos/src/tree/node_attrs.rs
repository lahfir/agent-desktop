use agent_desktop_core::node::Rect;

#[derive(Debug, Clone, Default)]
pub(crate) struct NodeAttrs {
    pub(crate) role: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) value: Option<String>,
    pub(crate) states: NodeAttrStates,
    pub(crate) bounds: Option<Rect>,
    pub(crate) has_scrollbars: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeAttrStates {
    pub(crate) enabled: bool,
    pub(crate) focused: Option<bool>,
    pub(crate) expanded: Option<bool>,
    pub(crate) disclosing: Option<bool>,
}

impl Default for NodeAttrStates {
    fn default() -> Self {
        Self {
            enabled: true,
            focused: None,
            expanded: None,
            disclosing: None,
        }
    }
}

pub(crate) fn parse_enabled(enabled: Option<String>) -> bool {
    enabled.map(|s| s == "true").unwrap_or(true)
}

pub(crate) fn parse_bool_attr(value: Option<String>) -> Option<bool> {
    value.map(|s| s == "true")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_defaults_to_true_when_missing() {
        assert!(parse_enabled(None));
    }

    #[test]
    fn enabled_accepts_true_string() {
        assert!(parse_enabled(Some("true".into())));
    }

    #[test]
    fn enabled_rejects_false_string() {
        assert!(!parse_enabled(Some("false".into())));
    }

    #[test]
    fn enabled_rejects_unknown_string() {
        assert!(!parse_enabled(Some("x".into())));
    }
}
