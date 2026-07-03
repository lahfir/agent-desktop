pub(crate) fn meaningful_native_id(raw: Option<String>) -> Option<String> {
    raw.filter(|id| !id.is_empty() && !is_auto_generated(id))
}

fn is_auto_generated(id: &str) -> bool {
    id.starts_with("_NS")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_generated_ns_prefix_is_filtered() {
        assert_eq!(meaningful_native_id(Some("_NS:42".into())), None);
    }

    #[test]
    fn developer_assigned_id_is_kept() {
        assert_eq!(
            meaningful_native_id(Some("submit-btn".into())),
            Some("submit-btn".into())
        );
    }
}
