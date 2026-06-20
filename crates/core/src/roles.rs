/// Interactive roles that receive refs during snapshot allocation.
///
/// Each entry must be produced by at least one platform adapter's native-to-canonical
/// role mapping. Read-only roles (statictext, image) and container roles (group, list,
/// table) stay out. Platform-private extensions live in the adapter, not here.
pub const INTERACTIVE_ROLES: &[&str] = &[
    "button",
    "cell",
    "checkbox",
    "colorwell",
    "combobox",
    "dockitem",
    "incrementor",
    "link",
    "menubutton",
    "menuitem",
    "radiobutton",
    "slider",
    "switch",
    "tab",
    "textfield",
    "treeitem",
];

/// Normalizes a caller-supplied role filter for comparison against tree
/// roles: trims, lowercases, and folds a few high-frequency web-automation
/// synonyms onto their canonical names so an agent's reflexive `textarea`
/// matches the `textfield` the adapters emit. This is an ergonomic shim,
/// not a vocabulary: it never rejects. Whether a role exists is answered
/// by the live tree (see `find`'s `roles_present`), so a new adapter role
/// works the instant the adapter emits it — nothing here to keep in sync.
pub fn normalize_role_query(role: &str) -> String {
    let normalized = role.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "textarea" | "textbox" | "searchfield" => "textfield".to_string(),
        _ => normalized,
    }
}

/// Returns true when `role` is in [`INTERACTIVE_ROLES`].
pub fn is_interactive_role(role: &str) -> bool {
    INTERACTIVE_ROLES.contains(&role)
}

/// Returns true for roles whose checked/unchecked state can be queried and set.
pub fn is_toggleable_role(role: &str) -> bool {
    matches!(role, "checkbox" | "switch" | "radiobutton")
}

/// Returns true for roles that carry an expanded/collapsed surface state.
pub fn is_expandable_role(role: &str) -> bool {
    matches!(role, "combobox" | "menubutton" | "treeitem" | "disclosure")
}

/// Returns true for roles whose `value` changes during normal interaction and
/// must not be treated as stable ref identity.
pub fn is_mutable_value_role(role: &str) -> bool {
    matches!(role, "combobox" | "incrementor" | "slider" | "textfield")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_roles_are_sorted_and_unique() {
        let mut sorted = INTERACTIVE_ROLES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.as_slice(), INTERACTIVE_ROLES);
    }

    #[test]
    fn normalize_role_query_folds_text_input_synonyms() {
        assert_eq!(normalize_role_query("textarea"), "textfield");
        assert_eq!(normalize_role_query("textbox"), "textfield");
        assert_eq!(normalize_role_query("searchfield"), "textfield");
    }

    #[test]
    fn normalize_role_query_is_case_insensitive_and_trimmed() {
        assert_eq!(normalize_role_query("Button"), "button");
        assert_eq!(normalize_role_query(" TEXTAREA "), "textfield");
    }

    #[test]
    fn normalize_role_query_passes_unknown_roles_through_untouched() {
        assert_eq!(normalize_role_query("navbar"), "navbar");
        assert_eq!(normalize_role_query("buttn"), "buttn");
    }

    #[test]
    fn toggleable_roles_are_a_subset_of_interactive() {
        for role in ["checkbox", "switch", "radiobutton"] {
            assert!(is_toggleable_role(role));
            assert!(is_interactive_role(role));
        }
        assert!(!is_toggleable_role("button"));
        assert!(!is_toggleable_role("textfield"));
    }

    #[test]
    fn interactive_expandable_roles_are_interactive() {
        for role in ["combobox", "menubutton", "treeitem"] {
            assert!(is_expandable_role(role));
            assert!(is_interactive_role(role));
        }
        assert!(is_expandable_role("disclosure"));
        assert!(!is_interactive_role("disclosure"));
        assert!(!is_expandable_role("button"));
        assert!(!is_expandable_role("checkbox"));
    }

    #[test]
    fn interactive_role_expandables_are_in_interactive_roles() {
        for role in ["combobox", "menubutton", "treeitem"] {
            assert!(
                is_expandable_role(role),
                "{role} expected expandable for subset check"
            );
            assert!(
                INTERACTIVE_ROLES.contains(&role),
                "expandable role {role} missing from INTERACTIVE_ROLES"
            );
        }
    }

    #[test]
    fn every_toggleable_role_is_interactive() {
        for role in ["checkbox", "switch", "radiobutton"] {
            assert!(is_toggleable_role(role));
            assert!(
                INTERACTIVE_ROLES.contains(&role),
                "toggleable role {role} missing from INTERACTIVE_ROLES"
            );
        }
    }

    #[test]
    fn read_only_roles_are_never_interactive() {
        for role in ["statictext", "image", "group", "list", "table"] {
            assert!(!is_interactive_role(role));
        }
    }

    #[test]
    fn mutable_value_roles_are_interactive() {
        for role in ["combobox", "incrementor", "slider", "textfield"] {
            assert!(is_mutable_value_role(role));
            assert!(is_interactive_role(role));
        }
        assert!(!is_mutable_value_role("cell"));
        assert!(!is_mutable_value_role("button"));
    }
}
