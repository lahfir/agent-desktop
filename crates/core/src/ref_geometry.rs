use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefGeometry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<crate::Rect>,
    pub bounds_hash: Option<u64>,
}
