use super::LocatorField;
use crate::{NodeDescriptor, Rect};

#[derive(Debug, Clone, PartialEq)]
pub struct LocatorRefEvidence {
    pub bounds: LocatorField<Rect>,
    pub available_actions: LocatorField<Vec<String>>,
    /// The presentation-only descriptor group, carried here as its
    /// evidence-side home: `LocatorEvidence` and `ObservedNode` are both at
    /// the field cap, so the group rides the observed-node projection here.
    pub descriptors: NodeDescriptor,
}
