use super::{
    NodeAttrs, node_attribute_status::NodeAttributeStatus, node_identifiers::NodeIdentifiers,
};

pub(crate) struct NodeAttributeRead {
    pub(crate) attrs: NodeAttrs,
    pub(crate) identifiers: NodeIdentifiers,
    pub(crate) metrics: super::node_attribute_metrics::NodeAttributeMetrics,
    pub(crate) status: NodeAttributeStatus,
}
