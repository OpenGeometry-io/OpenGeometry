use crate::brep::BrepEnvelope;
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) type ShapeId = String;

#[derive(Clone, Debug)]
pub struct Shape {
    pub brep: Arc<BrepEnvelope>,
    pub revision: u64,
    pub users: u32,
    pub edge_keys: Vec<String>,
    pub report: Option<serde_json::Value>,
}

#[derive(Default)]
pub(super) struct ShapeStore {
    pub(super) shapes: BTreeMap<ShapeId, Shape>,
    pub(super) high_water: BTreeMap<ShapeId, u64>,
    pub(super) next_id: u64,
}

#[derive(Clone, Debug)]
pub struct Reserved {
    pub shape_ids: Vec<ShapeId>,
    pub(super) revisions: Vec<(ShapeId, u64)>,
}
