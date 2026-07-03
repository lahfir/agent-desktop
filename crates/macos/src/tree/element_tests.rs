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

#[test]
fn readonly_derivation_has_a_single_owner() {
    let editable_role_check_sites = ELEMENT_SOURCE.matches("editable_ax_role(role)").count();
    assert_eq!(
        editable_role_check_sites, 1,
        "readonly derivation must be computed once in compute_readonly, not re-derived \
         separately by fetch_node_attrs and fetch_node_attrs_slow"
    );
}

#[test]
fn fetch_node_attrs_reuses_cached_attr_names_array() {
    assert!(
        ELEMENT_SOURCE.contains("ATTR_NAMES_CACHE.with"),
        "fetch_node_attrs must reuse the thread-local cached attribute-name CFArray instead of \
         rebuilding its 18 CFStrings on every call"
    );
    assert!(
        !ELEMENT_SOURCE.contains("attr_names.iter().map(|a| CFString::new(a))"),
        "fetch_node_attrs regressed to rebuilding the CFString attr-name array inline per call; \
         that construction belongs solely in AttrNamesCache::build"
    );
}
