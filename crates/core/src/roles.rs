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
    matches!(role, "combobox" | "menubutton" | "treeitem")
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
    fn toggleable_roles_are_a_subset_of_interactive() {
        for role in ["checkbox", "switch", "radiobutton"] {
            assert!(is_toggleable_role(role));
            assert!(is_interactive_role(role));
        }
        assert!(!is_toggleable_role("button"));
        assert!(!is_toggleable_role("textfield"));
    }

    #[test]
    fn expandable_roles_are_a_subset_of_interactive() {
        for role in ["combobox", "menubutton", "treeitem"] {
            assert!(is_expandable_role(role));
            assert!(is_interactive_role(role));
        }
        assert!(!is_expandable_role("button"));
        assert!(!is_expandable_role("checkbox"));
        assert!(!is_expandable_role("disclosure"));
    }

    #[test]
    fn every_expandable_role_is_interactive() {
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
