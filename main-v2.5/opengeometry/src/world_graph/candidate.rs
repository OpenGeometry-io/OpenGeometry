use super::error::{ErrorCode, GraphError};
use super::node::{Node, NodeKind};
use super::shape_store::{Shape, ShapeId};
use crate::brep::{BrepEnvelope, Similarity3};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct Candidate {
    pub(super) nodes: BTreeMap<String, Node>,
    pub(super) shapes: BTreeMap<ShapeId, Shape>,
    pub(super) next_shape_id: u64,
    pub(super) next_handle: u32,
    pub(super) next_generation: u32,
    pub(super) next_og_id: u64,
    pub(super) affected: BTreeSet<String>,
}

impl Candidate {
    pub(super) fn new_og_id(&mut self, supplied: Option<&str>) -> Result<String, GraphError> {
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
            let id = format!("node-{}", self.next_og_id);
            self.next_og_id = self.next_og_id.checked_add(1).ok_or_else(|| {
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
