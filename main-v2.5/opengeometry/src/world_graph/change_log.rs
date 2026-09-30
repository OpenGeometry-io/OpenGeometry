use super::shape_store::ShapeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeChange {
    pub og_id: String,
    pub handle: u32,
    pub(super) generation: u32,
    pub(super) shape_id: Option<ShapeId>,
    pub shape_revision: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeSet {
    pub(super) revision: u64,
    pub added: Vec<NodeChange>,
    pub changed: Vec<NodeChange>,
    pub removed: Vec<NodeChange>,
}
