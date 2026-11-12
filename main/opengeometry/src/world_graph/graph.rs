use super::candidate::IdCounters;
use super::change_log::ChangeSet;
use super::error::{ErrorCode, GraphError};
use super::node::{MarkId, Node};
use super::shape_store::{Shape, ShapeId, ShapeStore};
use crate::brep::{Accuracy, Similarity3};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

pub(super) struct JournalEntry {
    pub(super) nodes: BTreeMap<String, Option<Node>>,
    pub(super) shapes: BTreeMap<ShapeId, Option<Shape>>,
}

pub(super) struct LiveMark {
    pub(super) id: MarkId,
    pub(super) position: usize,
}

pub struct WorldGraph {
    pub(super) accuracy: Accuracy,
    pub(super) shapes: ShapeStore,
    pub(super) nodes: BTreeMap<String, Node>,
    pub(super) handles: HashMap<(u32, u32), String>,
    pub(super) revision: u64,
    pub(super) changes: Vec<ChangeSet>,
    pub(super) journal: Vec<JournalEntry>,
    pub(super) marks: Vec<LiveMark>,
    pub(super) next_mark: u64,
    pub(super) next_handle: u32,
    pub(super) next_generation: u32,
    pub(super) next_og_ids: IdCounters,
    pub(super) world_cache: RefCell<BTreeMap<String, Similarity3>>,
}

impl WorldGraph {
    pub fn new(accuracy: Accuracy) -> Result<Self, GraphError> {
        accuracy.validate()?;
        check_standard(accuracy)?;
        Ok(Self {
            accuracy,
            shapes: ShapeStore::default(),
            nodes: BTreeMap::new(),
            handles: HashMap::new(),
            revision: 0,
            changes: Vec::new(),
            journal: Vec::new(),
            marks: Vec::new(),
            next_mark: 0,
            next_handle: 0,
            next_generation: 0,
            next_og_ids: IdCounters::FIRST,
            world_cache: RefCell::new(BTreeMap::new()),
        })
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn accuracy(&self) -> Accuracy {
        self.accuracy
    }

    pub fn shape_count(&self) -> usize {
        self.shapes.shapes.len()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn insert_node(&mut self, og_id: String, node: Node) {
        let key = (node.handle, node.generation);
        if let Some(previous) = self.nodes.insert(og_id.clone(), node) {
            self.handles.remove(&(previous.handle, previous.generation));
        }
        self.handles.insert(key, og_id);
    }

    pub(super) fn remove_node(&mut self, og_id: &str) {
        if let Some(previous) = self.nodes.remove(og_id) {
            self.handles.remove(&(previous.handle, previous.generation));
        }
    }
}

fn check_standard(accuracy: Accuracy) -> Result<(), GraphError> {
    let standard = Accuracy::STANDARD;
    let equal = [
        (accuracy.geometric, standard.geometric),
        (accuracy.intersection, standard.intersection),
        (accuracy.tessellation, standard.tessellation),
        (accuracy.exchange, standard.exchange),
    ]
    .iter()
    .all(|(value, expected)| value.to_bits() == expected.to_bits());
    if equal {
        Ok(())
    } else {
        Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "accuracy must be the standard value",
        ))
    }
}
