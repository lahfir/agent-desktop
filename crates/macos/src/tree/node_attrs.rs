#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NodeAttrs {
    pub(crate) role: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) value: Option<String>,
    pub(crate) enabled: bool,
}

impl NodeAttrs {
    pub(crate) fn with_enabled_default(mut attrs: Self, enabled: Option<String>) -> Self {
        attrs.enabled = enabled.map(|s| s == "true").unwrap_or(true);
        attrs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_defaults_to_true_when_missing() {
        let attrs = NodeAttrs::with_enabled_default(NodeAttrs::default(), None);

        assert!(attrs.enabled);
    }

    #[test]
    fn enabled_accepts_true_string() {
        let attrs = NodeAttrs::with_enabled_default(NodeAttrs::default(), Some("true".into()));

        assert!(attrs.enabled);
    }

    #[test]
    fn enabled_rejects_false_string() {
        let attrs = NodeAttrs::with_enabled_default(NodeAttrs::default(), Some("false".into()));

        assert!(!attrs.enabled);
    }

    #[test]
    fn enabled_rejects_unknown_string() {
        let attrs = NodeAttrs::with_enabled_default(NodeAttrs::default(), Some("x".into()));

        assert!(!attrs.enabled);
    }
}
