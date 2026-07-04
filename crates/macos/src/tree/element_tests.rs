const ELEMENT_SOURCE: &str = include_str!("element.rs");

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
