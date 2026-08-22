#[derive(Clone, Copy)]
pub(crate) struct ChildReadPlan {
    max_elements: usize,
    boundary_elements: usize,
    logical_depth: Option<u8>,
    max_logical_depth: u8,
}

impl ChildReadPlan {
    pub(crate) fn load(max_elements: usize) -> Self {
        Self {
            max_elements,
            boundary_elements: max_elements,
            logical_depth: None,
            max_logical_depth: u8::MAX,
        }
    }

    pub(crate) fn boundary_aware(
        max_elements: usize,
        boundary_elements: usize,
        logical_depth: u8,
        max_logical_depth: u8,
    ) -> Self {
        Self {
            max_elements,
            boundary_elements,
            logical_depth: Some(logical_depth),
            max_logical_depth,
        }
    }

    pub(crate) fn max_elements(self, transparent_wrapper: bool) -> usize {
        let beyond_boundary = self.logical_depth.is_some_and(|depth| {
            depth.saturating_add(u8::from(!transparent_wrapper)) > self.max_logical_depth
        });
        if beyond_boundary {
            self.boundary_elements
        } else {
            self.max_elements
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_nodes_request_only_the_native_child_count() {
        let plan = ChildReadPlan::boundary_aware(128, 0, 3, 3);

        assert_eq!(plan.max_elements(false), 0);
        assert_eq!(plan.max_elements(true), 128);
    }

    #[test]
    fn selected_root_boundary_can_load_only_bounded_label_children() {
        let plan = ChildReadPlan::boundary_aware(128, 5, 0, 0);

        assert_eq!(plan.max_elements(false), 5);
    }

    #[test]
    fn actionable_group_cannot_load_past_the_boundary() {
        let plan = ChildReadPlan::boundary_aware(4_096, 0, 3, 3);
        let transparent = super::super::node_evidence::is_transparent_wrapper(
            Some("AXGroup"),
            None,
            &agent_desktop_core::NameEvidence::default(),
            None,
            &agent_desktop_core::IdentifierEvidence::absent(),
            &agent_desktop_core::LocatorField::Known(vec![
                agent_desktop_core::capability::CLICK.into(),
            ]),
        );

        assert!(!transparent);
        assert_eq!(plan.max_elements(transparent), 0);
    }
}
