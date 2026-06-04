use super::{AXElement, same_element};

#[derive(Default)]
pub(crate) struct ElementDedupe;

impl ElementDedupe {
    pub(crate) fn push(&mut self, elements: &mut Vec<AXElement>, element: AXElement) -> bool {
        if elements
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
    fn null_elements_do_not_collapse_without_semantic_identity() {
        let mut dedupe = ElementDedupe;
        let mut elements = Vec::new();

        assert!(dedupe.push(&mut elements, null_element()));
        assert!(dedupe.push(&mut elements, null_element()));
        assert_eq!(elements.len(), 2);
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
