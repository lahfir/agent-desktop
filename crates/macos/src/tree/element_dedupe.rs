use rustc_hash::FxHashSet;

use super::{AXElement, same_element};

#[derive(Default)]
pub(crate) struct ElementDedupe {
    pointer_keys: FxHashSet<usize>,
}

impl ElementDedupe {
    pub(crate) fn push(&mut self, elements: &mut Vec<AXElement>, element: AXElement) -> bool {
        let pointer_key = element.0 as usize;
        if !self.pointer_keys.insert(pointer_key) {
            return false;
        }
        if pointer_key != 0
            && elements
                .iter()
                .any(|existing| equivalent(existing, &element))
        {
            return false;
        }
        elements.push(element);
        true
    }

    pub(crate) fn push_clone(
        &mut self,
        elements: &mut Vec<AXElement>,
        element: &AXElement,
    ) -> bool {
        self.push(elements, element.clone())
    }
}

fn equivalent(existing: &AXElement, element: &AXElement) -> bool {
    existing.0 as usize != 0 && same_element(existing, element)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_duplicates_are_collapsed_without_semantic_lookup() {
        let mut dedupe = ElementDedupe::default();
        let mut elements = Vec::new();

        assert!(dedupe.push(&mut elements, null_element()));
        assert!(!dedupe.push(&mut elements, null_element()));
        assert_eq!(elements.len(), 1);
    }

    #[cfg(target_os = "macos")]
    fn null_element() -> AXElement {
        AXElement(std::ptr::null_mut())
    }

    #[cfg(not(target_os = "macos"))]
    fn null_element() -> AXElement {
        AXElement(std::ptr::null())
    }
}
