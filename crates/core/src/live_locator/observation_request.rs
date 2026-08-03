use crate::{
    AdapterError, Deadline, ErrorCode, adapter::TreeOptions, locator::LocatorQuery,
    snapshot_surface::SnapshotSurface,
};

use super::{
    EvidenceRequirements, LocatorResolveRequest, ObservationBudget, evidence_plan::EvidencePlan,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservationRequest {
    pub deadline: Deadline,
    pub max_raw_depth: u8,
    pub max_logical_depth: u8,
    pub surface: SnapshotSurface,
    pub skeleton: bool,
    evidence_plan: EvidencePlan,
    pub budget: ObservationBudget,
    pub observation_mode: ObservationMode,
}

/// The observation-mode sub-struct: how the caller wants renderer
/// accessibility handled on a web-wrapped target. The Windows adapter reads it
/// to decide whether a still-thin post-settle tree demands the
/// `--force-renderer-accessibility` guidance or a bare tree back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObservationMode {
    /// The caller will pass Chromium's `--force-renderer-accessibility`
    /// (via the `--force-electron-a11y` CLI flag), so the adapter should not
    /// guess at guidance; it returns the tree it observed.
    pub force_renderer_accessibility: bool,
}

impl ObservationRequest {
    pub fn validate(self) -> Result<Self, AdapterError> {
        self.budget.validate()?;
        if !(1..=50).contains(&self.max_raw_depth) {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "max_raw_depth must be between 1 and 50",
            ));
        }
        if self.max_logical_depth > self.max_raw_depth {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "max_logical_depth cannot exceed max_raw_depth",
            ));
        }
        if self.skeleton && self.max_logical_depth > 3 {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "skeleton observations support a maximum logical depth of 3",
            ));
        }
        self.evidence_plan.validate()?;
        Ok(self)
    }

    pub fn snapshot(options: &TreeOptions, deadline: Deadline) -> Self {
        Self {
            deadline,
            max_raw_depth: 50,
            max_logical_depth: if options.skeleton {
                options.max_depth.min(3)
            } else {
                options.max_depth
            },
            surface: options.surface,
            skeleton: options.skeleton,
            evidence_plan: EvidencePlan::uniform(EvidenceRequirements::snapshot()),
            budget: ObservationBudget::default(),
            observation_mode: ObservationMode {
                force_renderer_accessibility: options.force_renderer_accessibility,
            },
        }
    }

    pub fn locator(
        query: &LocatorQuery,
        request: &LocatorResolveRequest,
        deadline: Deadline,
    ) -> Self {
        Self {
            deadline,
            max_raw_depth: request.max_raw_depth,
            max_logical_depth: request.max_raw_depth,
            surface: SnapshotSurface::Window,
            skeleton: false,
            evidence_plan: EvidencePlan::uniform(EvidenceRequirements::locator(query, request)),
            budget: ObservationBudget::default(),
            observation_mode: ObservationMode::default(),
        }
    }

    pub(crate) fn locator_for_root(
        query: &LocatorQuery,
        request: &LocatorResolveRequest,
        root: super::ObservationRoot<'_>,
        deadline: Deadline,
    ) -> Self {
        Self {
            surface: root.surface(),
            ..Self::locator(query, request, deadline)
        }
    }

    pub(crate) fn selected_hydration(
        query: &LocatorQuery,
        request: &LocatorResolveRequest,
        root: super::ObservationRoot<'_>,
        deadline: Deadline,
    ) -> Self {
        let subtree_query = query.has_text.is_some()
            || query.containment.has.is_some()
            || query.containment.has_not.is_some();
        Self {
            deadline,
            max_raw_depth: if subtree_query {
                request.max_raw_depth
            } else {
                1
            },
            max_logical_depth: if subtree_query {
                request.max_raw_depth
            } else {
                0
            },
            surface: root.surface(),
            skeleton: false,
            evidence_plan: EvidencePlan::rooted(
                EvidenceRequirements::snapshot(),
                EvidenceRequirements::query(query),
            ),
            budget: ObservationBudget::default(),
            observation_mode: ObservationMode::default(),
        }
    }

    pub fn evidence_for_raw_depth(self, raw_depth: u8) -> EvidenceRequirements {
        self.evidence_plan.for_raw_depth(raw_depth)
    }

    /// Sets the observation-mode sub-struct the adapter reads for
    /// renderer-accessibility handling on web-wrapped targets.
    pub fn with_observation_mode(mut self, mode: ObservationMode) -> Self {
        self.observation_mode = mode;
        self
    }

    pub fn descendant_evidence(self) -> EvidenceRequirements {
        self.evidence_plan.descendants()
    }

    pub fn hydrates_root_name_from_children(self) -> bool {
        self.evidence_plan.hydrates_root_name_from_children()
    }
}
