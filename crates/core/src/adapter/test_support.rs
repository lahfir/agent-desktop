macro_rules! complete_live_observation {
    ($role:expr, $name:expr, [$($action:expr),* $(,)?]) => {
        fn get_live_element(
            &self,
            _handle: &$crate::adapter::NativeHandle,
            _deadline: $crate::Deadline,
        ) -> Result<$crate::LiveElement, $crate::AdapterError> {
            Ok($crate::LiveElement {
                identity: $crate::LiveIdentity {
                    name: $crate::LocatorField::Known($name.into()),
                    description: $crate::LocatorField::Absent,
                    identifiers: $crate::IdentifierEvidence::absent(),
                },
                state: $crate::ElementState {
                    role: $role.into(),
                    states: Vec::new(),
                    value: None,
                    enabled: Some(true),
                    hidden: Some(false),
                    offscreen: Some(false),
                },
                states_complete: true,
                bounds: Some($crate::Rect {
                    x: 1.0,
                    y: 1.0,
                    width: 20.0,
                    height: 20.0,
                }),
                available_actions: vec![$($action.into()),*],
            })
        }

        fn get_live_state(
            &self,
            _handle: &$crate::adapter::NativeHandle,
            _deadline: $crate::Deadline,
        ) -> Result<Option<$crate::ElementState>, $crate::AdapterError> {
            Ok(Some($crate::ElementState {
                role: $role.into(),
                states: Vec::new(),
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            }))
        }

        fn get_element_bounds(
            &self,
            _handle: &$crate::adapter::NativeHandle,
            _deadline: $crate::Deadline,
        ) -> Result<Option<$crate::Rect>, $crate::AdapterError> {
            Ok(Some($crate::Rect {
                x: 1.0,
                y: 1.0,
                width: 20.0,
                height: 20.0,
            }))
        }

        fn get_live_actions(
            &self,
            _handle: &$crate::adapter::NativeHandle,
            _deadline: $crate::Deadline,
        ) -> Result<Option<Vec<String>>, $crate::AdapterError> {
            Ok(Some(vec![$($action.into()),*]))
        }

        fn hit_test(
            &self,
            _handle: &$crate::adapter::NativeHandle,
            _point: $crate::Point,
            _deadline: $crate::Deadline,
        ) -> Result<$crate::hit_test::HitTestResult, $crate::AdapterError> {
            Ok($crate::hit_test::HitTestResult::ReachesTarget)
        }
    };
}

pub(crate) use complete_live_observation;

macro_rules! guarded_interaction_lease {
    () => {
        fn acquire_interaction_lease(
            &self,
            deadline: $crate::Deadline,
        ) -> Result<$crate::InteractionLease, $crate::AdapterError> {
            $crate::InteractionLease::guarded(deadline, ())
        }
    };
}

pub(crate) use guarded_interaction_lease;

macro_rules! exact_window_focus {
    () => {
        fn resolve_window_strict(
            &self,
            window: &$crate::WindowInfo,
            _deadline: $crate::Deadline,
        ) -> Result<$crate::WindowInfo, $crate::AdapterError> {
            Ok(window.clone())
        }

        fn focus_window(
            &self,
            _window: &$crate::WindowInfo,
            _lease: &$crate::InteractionLease,
        ) -> Result<(), $crate::AdapterError> {
            Ok(())
        }
    };
}

pub(crate) use exact_window_focus;

pub(crate) fn live_identity(name: &str) -> crate::LiveIdentity {
    crate::LiveIdentity {
        name: crate::LocatorField::Known(name.into()),
        description: crate::LocatorField::Absent,
        identifiers: crate::IdentifierEvidence::absent(),
    }
}

pub(crate) fn observed_tree(
    root: &crate::live_locator::ObservationRoot<'_>,
    node: crate::AccessibilityNode,
) -> Result<crate::live_locator::ObservedTree, crate::AdapterError> {
    use crate::live_locator::{
        IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence, LocatorStats,
        ObservationSource, ObservedSubtree, ObservedTree,
    };

    fn field(value: Option<String>) -> LocatorField<String> {
        value
            .map(LocatorField::Known)
            .unwrap_or(LocatorField::Absent)
    }

    fn subtree(node: crate::AccessibilityNode) -> ObservedSubtree {
        let native_id = node.identity.native_id.clone();
        let evidence = LocatorEvidence {
            role: LocatorField::Known(node.role),
            name: field(node.identity.name),
            description: field(node.identity.description),
            value: field(node.identity.value),
            identifiers: IdentifierEvidence::typed(native_id, Some(0), true),
            states: LocatorField::Known(node.presentation.states),
            ref_evidence: LocatorRefEvidence {
                bounds: node
                    .presentation
                    .bounds
                    .map(LocatorField::Known)
                    .unwrap_or(LocatorField::Absent),
                available_actions: LocatorField::Known(node.presentation.available_actions),
                descriptors: node.presentation.descriptors.clone(),
            },
        };
        let children = node.children.into_iter().map(subtree).collect();
        ObservedSubtree::new(evidence, children, true, node.children_count)
    }

    ObservedTree::from_roots(
        vec![subtree(node)],
        ObservationSource::from_root(root),
        LocatorStats::default(),
        true,
    )
}
