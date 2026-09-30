use super::shape_store::ShapeId;
use crate::brep::Similarity3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum NodeKind {
    SystemAssembly,
    Body,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EditScope {
    Node,
    AllInstances,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub og_id: String,
    pub handle: u32,
    pub generation: u32,
    pub(crate) parent: Option<String>,
    pub(crate) children: Vec<String>,
    pub local: Similarity3,
    pub shape: Option<ShapeId>,
    pub(crate) kind: NodeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkId(pub(super) u64);
