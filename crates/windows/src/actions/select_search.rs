//! Bounded SelectionItem descendant search and scroll-to-realize (A19-7).

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use crate::tree::element::UIAElement;

pub(crate) const MAX_SELECT_NODES: usize = 2_048;
pub(crate) const MAX_SELECT_DEPTH: u8 = 8;
const MAX_REALIZE_SCROLLS: u32 = 16;

/// Second same-name SelectionItem is `AMBIGUOUS_TARGET`, never a silent pick.
pub(crate) fn accept_selection_match(already_found: bool) -> Result<(), AdapterError> {
    if already_found {
        Err(AdapterError::ambiguous_target(
            "Multiple SelectionItem elements share the requested accessible name",
        )
        .with_details(serde_json::json!({
            "kind": "ambiguous_select_value",
        })))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        AdapterError, Deadline, ErrorCode, MAX_REALIZE_SCROLLS, MAX_SELECT_DEPTH, MAX_SELECT_NODES,
        UIAElement, accept_selection_match,
    };
    use crate::actions::mutation::{classify_success, classify_write};
    use crate::actions::scroll::SCROLL_LABEL;
    use crate::system::permissions::ensure_budget;
    use crate::tree::automation::uia_failure_error;
    use crate::tree::element_properties::ElementProperties;
    use crate::tree::name_evidence::{name_fields, read_label};
    use crate::tree::properties::read_one;
    use crate::tree::property_ids::TreeProperty;
    use crate::tree::walker::TreeSource;
    use crate::tree::walker_source::UiaTreeSource;
    use agent_desktop_core::LocatorField;
    use uiautomation::patterns::UIScrollPattern;
    use uiautomation::types::ScrollAmount;

    pub(crate) fn find_named_selection_item(
        root: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<Option<UIAElement>, AdapterError> {
        let source = UiaTreeSource::for_root(root)?;
        let prepared = source.prepare_root(root)?;
        let mut stack = Vec::new();
        push_children(&source, &prepared, 1, &mut stack)?;
        let mut visited = 0_usize;
        let mut found: Option<UIAElement> = None;
        while let Some((candidate, depth)) = stack.pop() {
            ensure_budget(deadline)?;
            visited = visited.saturating_add(1);
            if visited > MAX_SELECT_NODES {
                return Err(node_budget_error());
            }
            if selection_item_available(&candidate) && name_matches(&candidate, value) {
                accept_selection_match(found.is_some())?;
                found = Some(candidate.clone());
            }
            if depth < MAX_SELECT_DEPTH {
                push_children(&source, &candidate, depth.saturating_add(1), &mut stack)?;
            }
        }
        Ok(found)
    }

    pub(crate) fn scroll_to_realize(
        element: &UIAElement,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        if read_one(element, TreeProperty::ScrollAvailable).flag() != Some(true) {
            return Ok(());
        }
        let Ok(pattern) = element.0.get_pattern::<UIScrollPattern>() else {
            return Ok(());
        };
        if !pattern.is_vertically_scrollable().unwrap_or(false) {
            return Ok(());
        }
        for _ in 0..MAX_REALIZE_SCROLLS {
            ensure_budget(deadline)?;
            let before = pattern.get_vertical_scroll_percent().ok();
            match pattern.scroll(ScrollAmount::NoAmount, ScrollAmount::SmallIncrement) {
                Ok(()) => {
                    let _ = classify_success()?;
                }
                Err(error) => {
                    let _ = classify_write("Scroll", SCROLL_LABEL, &error)?;
                    break;
                }
            }
            let after = pattern.get_vertical_scroll_percent().ok();
            if before.is_some() && before == after {
                break;
            }
        }
        Ok(())
    }

    pub(crate) fn selection_item_available(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::SelectionItemAvailable).flag() == Some(true)
    }

    pub(crate) fn name_matches(element: &UIAElement, value: &str) -> bool {
        let properties = ElementProperties::from_reads(vec![
            (TreeProperty::Name, read_one(element, TreeProperty::Name)),
            (
                TreeProperty::FullDescription,
                read_one(element, TreeProperty::FullDescription),
            ),
            (
                TreeProperty::HelpText,
                read_one(element, TreeProperty::HelpText),
            ),
            (
                TreeProperty::IsPassword,
                read_one(element, TreeProperty::IsPassword),
            ),
        ]);
        let label = read_label(element, false);
        let (name, _) = name_fields(&properties, &label);
        matches!(
            name,
            LocatorField::Known(text) if text.eq_ignore_ascii_case(value)
        )
    }

    fn push_children(
        source: &UiaTreeSource,
        parent: &UIAElement,
        child_depth: u8,
        stack: &mut Vec<(UIAElement, u8)>,
    ) -> Result<(), AdapterError> {
        let mut child = match source.first_child(parent) {
            Ok(child) => Some(child),
            Err(failure) if failure.is_exhaustion() => None,
            Err(failure) => {
                return Err(uia_failure_error(failure, "enumerate children for select"));
            }
        };
        let mut siblings = Vec::new();
        while let Some(current) = child {
            let next = match source.next_sibling(&current) {
                Ok(next) => Some(next),
                Err(failure) if failure.is_exhaustion() => None,
                Err(failure) => {
                    return Err(uia_failure_error(failure, "enumerate siblings for select"));
                }
            };
            siblings.push(current);
            child = next;
        }
        for sibling in siblings.into_iter().rev() {
            stack.push((sibling, child_depth));
        }
        Ok(())
    }

    fn node_budget_error() -> AdapterError {
        AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Select search exceeded its accessibility-node budget",
        )
        .with_details(serde_json::json!({
            "kind": "select_node_limit",
            "limit": MAX_SELECT_NODES,
            "complete": false,
        }))
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{AdapterError, Deadline, UIAElement};

    pub(crate) fn find_named_selection_item(
        _root: &UIAElement,
        _value: &str,
        _deadline: Deadline,
    ) -> Result<Option<UIAElement>, AdapterError> {
        Ok(None)
    }

    pub(crate) fn scroll_to_realize(
        _element: &UIAElement,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    pub(crate) fn selection_item_available(_element: &UIAElement) -> bool {
        false
    }

    pub(crate) fn name_matches(_element: &UIAElement, _value: &str) -> bool {
        false
    }
}

pub(crate) use imp::{
    find_named_selection_item, name_matches, scroll_to_realize, selection_item_available,
};

#[cfg(test)]
mod tests {
    use super::{MAX_SELECT_DEPTH, accept_selection_match};
    use agent_desktop_core::ErrorCode;

    #[test]
    fn select_search_depth_budget_is_eight() {
        assert_eq!(MAX_SELECT_DEPTH, 8);
    }

    #[test]
    fn first_selection_match_is_accepted() {
        accept_selection_match(false).expect("first");
    }

    #[test]
    fn second_selection_match_is_ambiguous() {
        let error = accept_selection_match(true).expect_err("duplicate");
        assert_eq!(error.code, ErrorCode::AmbiguousTarget);
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("kind"))
                .and_then(serde_json::Value::as_str),
            Some("ambiguous_select_value")
        );
    }
}
