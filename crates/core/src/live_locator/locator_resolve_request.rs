use super::{LocatorMaterialization, LocatorSelection};
use crate::Deadline;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocatorResolveRequest {
    pub selection: LocatorSelection,
    pub deadline: Deadline,
    pub max_raw_depth: u8,
    pub materialization: LocatorMaterialization,
}
