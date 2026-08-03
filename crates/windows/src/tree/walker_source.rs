use agent_desktop_core::{AdapterError, ObservationRoot};

use super::element::UIAElement;
use super::walker::{WalkBudget, WalkOutcome, walk_from_root};

#[cfg(target_os = "windows")]
mod imp {
    use crate::tree::automation::{UiaFailure, automation_client, failure_of, uia_error};
    use crate::tree::cache::{build_walk_cache_request, policy_for_root};
    use crate::tree::element::UIAElement;
    use crate::tree::name_evidence::read_label;
    use crate::tree::properties::{read_cached, read_live};
    use crate::tree::walker::{NodeKey, TreeSource, walk_vocabulary};
    use agent_desktop_core::AdapterError;
    use uiautomation::UIAutomation;
    use uiautomation::core::{UICacheRequest, UITreeWalker};

    /// The production enumeration surface: the raw view walker, plus the cache
    /// request the provider class earned.
    pub struct UiaTreeSource {
        client: UIAutomation,
        walker: UITreeWalker,
        cache: Option<UICacheRequest>,
        /// Whether the root is Chromium/Electron provenance (the provenance
        /// gate): the transparent-wrapper depth skip must only fire under
        /// detected Chromium/WebView2 provenance, because the identical
        /// emptiness test would otherwise skip the anonymous `Group`/`Pane`
        /// containers native stacks are full of.
        chromium_provenance: bool,
    }

    impl UiaTreeSource {
        pub fn for_root(root: &UIAElement) -> Result<Self, AdapterError> {
            let client = automation_client()?;
            let walker = client
                .get_raw_view_walker()
                .map_err(|error| uia_error(&error, "open the raw view tree walker"))?;
            let cache = if policy_for_root(root).batches() {
                Some(build_walk_cache_request(&client)?)
            } else {
                None
            };
            Ok(Self {
                client,
                walker,
                cache,
                chromium_provenance: crate::tree::chromium::is_chromium_root(root),
            })
        }

        /// Prefetches the walk property set onto the caller's root.
        ///
        /// The root arrives from `ElementFromHandle` or from a stored ref, so
        /// it carries no cache of its own; without this the root alone would
        /// read `Unknown` for every property while its children read fine.
        pub fn prepare_root(&self, root: &UIAElement) -> Result<UIAElement, AdapterError> {
            match &self.cache {
                Some(request) => root
                    .0
                    .build_updated_cache(request)
                    .map(UIAElement::from)
                    .map_err(|error| {
                        uia_error(&error, "prefetch the properties of a root element")
                    }),
                None => Ok(root.clone()),
            }
        }

        /// Whether the root this source walks carries Chromium provenance; the
        /// wrapper-depth skip only fires when this is true.
        #[cfg(test)]
        pub(crate) fn chromium_provenance(&self) -> bool {
            self.chromium_provenance
        }
    }

    impl TreeSource for UiaTreeSource {
        type Node = UIAElement;

        fn first_child(&self, node: &UIAElement) -> Result<UIAElement, UiaFailure> {
            match &self.cache {
                Some(request) => self.walker.get_first_child_build_cache(&node.0, request),
                None => self.walker.get_first_child(&node.0),
            }
            .map(UIAElement::from)
            .map_err(|error| failure_of(&error))
        }

        fn next_sibling(&self, node: &UIAElement) -> Result<UIAElement, UiaFailure> {
            match &self.cache {
                Some(request) => self.walker.get_next_sibling_build_cache(&node.0, request),
                None => self.walker.get_next_sibling(&node.0),
            }
            .map(UIAElement::from)
            .map_err(|error| failure_of(&error))
        }

        fn identity(&self, node: &UIAElement) -> NodeKey {
            match node.0.get_runtime_id() {
                Ok(runtime_id) if !runtime_id.is_empty() => NodeKey::Runtime(runtime_id),
                _ => NodeKey::Unavailable,
            }
        }

        fn same_element(&self, left: &UIAElement, right: &UIAElement) -> bool {
            self.client
                .compare_elements(&left.0, &right.0)
                .unwrap_or(false)
        }

        fn evidence(
            &self,
            node: &UIAElement,
        ) -> (
            crate::tree::properties::ElementProperties,
            agent_desktop_core::LocatorEvidence,
            u64,
        ) {
            let (properties, errors) = match &self.cache {
                Some(_) => read_cached(node),
                None => read_live(node),
            };
            let failed = u64::try_from(errors.len()).unwrap_or(u64::MAX);
            let label = read_label(node, self.cache.is_some());
            let vocabulary = walk_vocabulary(&properties, &label);
            (
                properties.clone(),
                properties.into_locator_evidence(vocabulary),
                failed,
            )
        }

        fn is_web_wrapper(
            &self,
            _node: &UIAElement,
            properties: &crate::tree::properties::ElementProperties,
        ) -> bool {
            self.chromium_provenance && crate::tree::wrapper::is_web_wrapper(properties)
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use crate::tree::automation::{ERR_NONE, UiaFailure};
    use crate::tree::element::UIAElement;
    use crate::tree::element_properties::ResolvedVocabulary;
    use crate::tree::properties::ElementProperties;
    use crate::tree::walker::{NodeKey, TreeSource};
    use agent_desktop_core::AdapterError;

    /// Canned arm so the walk's entry point, and every module that calls it,
    /// compile and run on a non-Windows lane. It enumerates nothing, which is
    /// the exhaustion answer, so a walk here yields a single complete node.
    pub struct UiaTreeSource;

    impl UiaTreeSource {
        pub fn for_root(_root: &UIAElement) -> Result<Self, AdapterError> {
            Ok(Self)
        }

        pub fn prepare_root(&self, root: &UIAElement) -> Result<UIAElement, AdapterError> {
            Ok(root.clone())
        }
    }

    impl TreeSource for UiaTreeSource {
        type Node = UIAElement;

        fn first_child(&self, _node: &UIAElement) -> Result<UIAElement, UiaFailure> {
            Err(UiaFailure::Sentinel(ERR_NONE))
        }

        fn next_sibling(&self, _node: &UIAElement) -> Result<UIAElement, UiaFailure> {
            Err(UiaFailure::Sentinel(ERR_NONE))
        }

        fn identity(&self, _node: &UIAElement) -> NodeKey {
            NodeKey::Unavailable
        }

        fn same_element(&self, _left: &UIAElement, _right: &UIAElement) -> bool {
            false
        }

        fn evidence(
            &self,
            _node: &UIAElement,
        ) -> (
            crate::tree::properties::ElementProperties,
            agent_desktop_core::LocatorEvidence,
            u64,
        ) {
            (
                ElementProperties::default(),
                ElementProperties::default().into_locator_evidence(ResolvedVocabulary::unknown()),
                0,
            )
        }

        fn is_web_wrapper(
            &self,
            _node: &UIAElement,
            _properties: &crate::tree::properties::ElementProperties,
        ) -> bool {
            false
        }
    }
}

pub use imp::UiaTreeSource;

/// Walks a live UI Automation subtree from an arbitrary root element.
///
/// The cache policy is chosen once, from the root's provider class, and the
/// root is prefetched to match it, so the whole walk reads through one
/// consistent mechanism.
pub fn walk_uia_subtree(
    root: &UIAElement,
    root_source: &ObservationRoot<'_>,
    budget: WalkBudget,
) -> Result<WalkOutcome, AdapterError> {
    let source = UiaTreeSource::for_root(root)?;
    let prepared = source.prepare_root(root)?;
    walk_from_root(&source, &prepared, root_source, budget)
}

#[cfg(test)]
#[path = "walker_source_tests.rs"]
mod tests;
