const ELEMENT_SOURCE: &str = include_str!("element.rs");

#[test]
fn resolve_element_name_delegates_precedence_to_core_accname() {
    assert!(
        ELEMENT_SOURCE.contains("agent_desktop_core::accname::compute_name"),
        "resolve_element_name must reduce NameEvidence through core's compute_name (KTD6); \
         found no call to accname::compute_name in element.rs"
    );
    assert!(
        ELEMENT_SOURCE.contains("super::name_evidence::name_evidence_impl"),
        "resolve_element_name must gather evidence via the name_evidence supplier, not read \
         AX attributes itself"
    );
}

#[test]
fn resolve_element_name_owns_no_local_fallback_chain() {
    assert!(
        !ELEMENT_SOURCE.contains("title.or(desc)"),
        "resolve_element_name regressed to owning its own title/description fallback chain; \
         precedence belongs in accname::compute_name (KTD6)"
    );
    assert!(
        !ELEMENT_SOURCE.contains("kAXValueAttribute).or(name)"),
        "resolve_element_name regressed to owning its own static-role-value fallback; \
         precedence belongs in accname::compute_name (KTD6)"
    );
}
