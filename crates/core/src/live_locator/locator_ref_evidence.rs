use super::LocatorField;
use crate::Rect;

#[derive(Debug, Clone, PartialEq)]
pub struct LocatorRefEvidence {
    pub bounds: LocatorField<Rect>,
    pub available_actions: LocatorField<Vec<String>>,
}
