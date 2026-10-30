use super::error::{ErrorCode, GraphError};
use super::node::{Node, NodeKind};
use super::shape_store::{Shape, ShapeId};
use crate::brep::{BodyType, BrepEnvelope, Similarity3};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(super) struct IdCounters {
    solid: u64,
    sheet: u64,
    wire: u64,
    assembly: u64,
}

#[derive(Clone, Copy)]
pub(super) enum IdKind {
    Solid,
    Sheet,
    Wire,
    Assembly,
}

pub(super) struct Overlay<'a, V> {
    base: &'a BTreeMap<String, V>,
    pub(super) edits: BTreeMap<String, Option<V>>,
}

pub(super) struct Candidate<'a> {
    pub(super) nodes: Overlay<'a, Node>,
    pub(super) shapes: Overlay<'a, Shape>,
    pub(super) next_shape_id: u64,
    pub(super) next_handle: u32,
    pub(super) next_generation: u32,
    pub(super) next_og_ids: IdCounters,
    pub(super) affected: BTreeSet<String>,
}

pub(super) struct Edits {
    pub(super) nodes: BTreeMap<String, Option<Node>>,
    pub(super) shapes: BTreeMap<ShapeId, Option<Shape>>,
    pub(super) next_shape_id: u64,
    pub(super) next_handle: u32,
    pub(super) next_generation: u32,
    pub(super) next_og_ids: IdCounters,
    pub(super) affected: BTreeSet<String>,
}

impl<'a, V: Clone> Overlay<'a, V> {
    pub(super) fn new(base: &'a BTreeMap<String, V>) -> Self {
        Self {
            base,
            edits: BTreeMap::new(),
        }
    }

    pub(super) fn get(&self, id: &str) -> Option<&V> {
        match self.edits.get(id) {
            Some(edit) => edit.as_ref(),
            None => self.base.get(id),
        }
    }

    pub(super) fn contains_key(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    pub(super) fn get_mut(&mut self, id: &str) -> Option<&mut V> {
        if !self.edits.contains_key(id) {
            let entry = self.base.get(id)?.clone();
            self.edits.insert(id.to_string(), Some(entry));
        }
        self.edits.get_mut(id)?.as_mut()
    }

    pub(super) fn insert(&mut self, id: String, value: V) {
        self.edits.insert(id, Some(value));
    }

    pub(super) fn remove(&mut self, id: &str) -> Option<V> {
        let removed = match self.edits.remove(id) {
            Some(edit) => edit,
            None => self.base.get(id).cloned(),
        };
        self.edits.insert(id.to_string(), None);
        removed
    }
}

impl Candidate<'_> {
    pub(super) fn into_edits(self) -> Edits {
        Edits {
            nodes: self.nodes.edits,
            shapes: self.shapes.edits,
            next_shape_id: self.next_shape_id,
            next_handle: self.next_handle,
            next_generation: self.next_generation,
            next_og_ids: self.next_og_ids,
            affected: self.affected,
        }
    }

    pub(super) fn copy_kind(&self, shape_id: &str) -> Result<IdKind, GraphError> {
        let shape = self.shapes.get(shape_id).ok_or_else(|| {
            GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
        })?;
        IdKind::of_shape(&shape.brep)
    }

    pub(super) fn new_og_id(
        &mut self,
        supplied: Option<&str>,
        kind: IdKind,
    ) -> Result<String, GraphError> {
        if let Some(id) = supplied {
            if id.is_empty() {
                return Err(GraphError::code(
                    ErrorCode::InvalidParameter,
                    "ogId is empty",
                ));
            }
            if id.chars().count() > 1024 {
                return Err(GraphError::code(
                    ErrorCode::InvalidParameter,
                    "ogId exceeds 1024 characters",
                ));
            }
            if self.nodes.contains_key(id) {
                return Err(GraphError::code(ErrorCode::DuplicateNode, id));
            }
            return Ok(id.into());
        }
        loop {
            let counter = self.next_og_ids.slot(kind);
            let id = format!("{}-{}", kind.prefix(), *counter);
            *counter = counter.checked_add(1).ok_or_else(|| {
                GraphError::code(ErrorCode::LimitExceeded, "node id counter overflow")
            })?;
            if !self.nodes.contains_key(&id) {
                return Ok(id);
            }
        }
    }

    pub(super) fn new_shape_id(&mut self) -> Result<ShapeId, GraphError> {
        let id = format!("shape-{}", self.next_shape_id);
        self.next_shape_id = self.next_shape_id.checked_add(1).ok_or_else(|| {
            GraphError::code(ErrorCode::LimitExceeded, "shape id counter overflow")
        })?;
        Ok(id)
    }

    pub(super) fn new_handle(&mut self) -> Result<(u32, u32), GraphError> {
        let handle = self.next_handle;
        let generation = self.next_generation;
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .ok_or_else(|| GraphError::code(ErrorCode::LimitExceeded, "handle counter overflow"))?;
        self.next_generation = self.next_generation.checked_add(1).ok_or_else(|| {
            GraphError::code(ErrorCode::LimitExceeded, "generation counter overflow")
        })?;
        Ok((handle, generation))
    }

    pub(super) fn add_body(
        &mut self,
        id: &str,
        shape_id: ShapeId,
        brep: BrepEnvelope,
        edge_keys: Vec<String>,
        parent: &Option<String>,
        local: Similarity3,
    ) -> Result<(), GraphError> {
        let handle = self.new_handle()?;
        self.shapes.insert(
            shape_id.clone(),
            Shape {
                brep: Arc::new(brep),
                revision: 0,
                users: 1,
                edge_keys,
                report: None,
            },
        );
        self.add_node(id, handle, parent, local, Some(shape_id), NodeKind::Body);
        Ok(())
    }

    pub(super) fn add_node(
        &mut self,
        id: &str,
        handle: (u32, u32),
        parent: &Option<String>,
        local: Similarity3,
        shape: Option<ShapeId>,
        kind: NodeKind,
    ) {
        let (handle, generation) = handle;
        self.nodes.insert(
            id.to_string(),
            Node {
                og_id: id.to_string(),
                handle,
                generation,
                parent: parent.clone(),
                children: Vec::new(),
                local,
                shape,
                kind,
                place_input: None,
            },
        );
        if let Some(parent) = parent {
            if let Some(parent_node) = self.nodes.get_mut(parent) {
                parent_node.children.push(id.to_string());
            }
        }
        self.affected.insert(id.to_string());
    }
}

impl IdKind {
    pub(super) fn of_body(body_type: BodyType) -> Result<Self, GraphError> {
        match body_type {
            BodyType::Solid => Ok(Self::Solid),
            BodyType::Sheet => Ok(Self::Sheet),
            BodyType::Wire => Ok(Self::Wire),
        }
    }

    fn of_shape(brep: &BrepEnvelope) -> Result<Self, GraphError> {
        if !brep.solids.is_empty() {
            Ok(Self::Solid)
        } else if brep.topology.faces.is_empty() && !brep.topology.wires.is_empty() {
            Ok(Self::Wire)
        } else {
            Self::of_body(BodyType::Sheet)
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Sheet => "sheet",
            Self::Wire => "wire",
            Self::Assembly => "assembly",
        }
    }
}

impl IdCounters {
    pub(super) const FIRST: Self = Self {
        solid: 1,
        sheet: 1,
        wire: 1,
        assembly: 1,
    };

    fn slot(&mut self, kind: IdKind) -> &mut u64 {
        match kind {
            IdKind::Solid => &mut self.solid,
            IdKind::Sheet => &mut self.sheet,
            IdKind::Wire => &mut self.wire,
            IdKind::Assembly => &mut self.assembly,
        }
    }
}
