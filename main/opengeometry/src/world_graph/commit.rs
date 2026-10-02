use super::candidate::Candidate;
use super::change_log::{ChangeSet, NodeChange};
use super::error::{ErrorCode, GraphError};
use super::graph::{JournalEntry, WorldGraph};
use super::node::Node;
use super::shape_store::{Reserved, Shape, ShapeId};
use crate::brep::BrepEnvelope;
use crate::math::Point3;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

impl WorldGraph {
    pub fn reserve(&self, new_shapes: usize, changed: &[ShapeId]) -> Result<Reserved, GraphError> {
        let mut shape_ids = Vec::with_capacity(new_shapes);
        let mut next_id = self.shapes.next_id;
        while shape_ids.len() < new_shapes {
            let id = format!("shape-{next_id}");
            next_id = next_id.checked_add(1).ok_or_else(|| {
                GraphError::code(ErrorCode::LimitExceeded, "shape id counter overflow")
            })?;
            if !self.shapes.shapes.contains_key(&id) && !self.shapes.high_water.contains_key(&id) {
                shape_ids.push(id);
            }
        }
        let mut revisions = Vec::with_capacity(changed.len());
        for id in changed {
            let high = self.shapes.high_water.get(id).ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidOperand, "shape has no revision history")
            })?;
            let next = high.checked_add(1).ok_or_else(|| {
                GraphError::code(ErrorCode::LimitExceeded, "shape revision overflow")
            })?;
            revisions.push((id.clone(), next));
        }
        Ok(Reserved {
            shape_ids,
            revisions,
        })
    }

    fn draft(&self) -> Candidate {
        Candidate {
            nodes: self.nodes.clone(),
            shapes: self.shapes.shapes.clone(),
            next_shape_id: self.shapes.next_id,
            next_handle: self.next_handle,
            next_generation: self.next_generation,
            next_og_ids: self.next_og_ids,
            affected: BTreeSet::new(),
        }
    }

    pub(super) fn change_for(node: &Node, shapes: &BTreeMap<ShapeId, Shape>) -> NodeChange {
        NodeChange {
            og_id: node.og_id.clone(),
            handle: node.handle,
            generation: node.generation,
            shape_id: node.shape.clone(),
            shape_revision: node
                .shape
                .as_ref()
                .and_then(|id| shapes.get(id).map(|shape| shape.revision)),
        }
    }

    fn precision_guard(&self, brep: &BrepEnvelope) -> Result<(), GraphError> {
        brep.validate()?;
        if let Some(bounds) = brep.bounds()? {
            let corner: Point3 = std::array::from_fn(|axis| {
                bounds.axes[axis].lo.abs().max(bounds.axes[axis].hi.abs())
            });
            let magnitude = corner[0].hypot(corner[1]).hypot(corner[2]);
            if magnitude * 64.0 * f64::EPSILON > self.accuracy.geometric {
                return Err(GraphError::code(
                    ErrorCode::LimitExceeded,
                    "shape exceeds precision extent",
                ));
            }
        }
        Ok(())
    }

    fn commit(&mut self, draft: Candidate) -> Result<ChangeSet, GraphError> {
        self.check_changed_shapes(&draft)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| GraphError::code(ErrorCode::LimitExceeded, "graph revision overflow"))?;
        let mut before_nodes = BTreeMap::new();
        let mut before_shapes = BTreeMap::new();
        let mut changes = ChangeSet {
            revision,
            added: Vec::new(),
            changed: Vec::new(),
            removed: Vec::new(),
        };
        self.add_node_changes(&draft, &mut before_nodes, &mut changes);
        self.add_shape_changes(&draft, &mut before_shapes, &mut changes);
        if !self.marks.is_empty() {
            self.journal.push(JournalEntry {
                nodes: before_nodes,
                shapes: before_shapes,
            });
        }
        for (id, shape) in &draft.shapes {
            self.shapes
                .high_water
                .entry(id.clone())
                .and_modify(|high| *high = (*high).max(shape.revision))
                .or_insert(shape.revision);
        }
        self.nodes = draft.nodes;
        self.shapes.shapes = draft.shapes;
        self.shapes.next_id = draft.next_shape_id;
        self.next_handle = draft.next_handle;
        self.next_generation = draft.next_generation;
        self.next_og_ids = draft.next_og_ids;
        self.revision = revision;
        self.world_cache
            .borrow_mut()
            .retain(|id, _| !draft.affected.contains(id));
        self.changes.push(changes.clone());
        Ok(changes)
    }

    fn check_changed_shapes(&self, draft: &Candidate) -> Result<(), GraphError> {
        for (id, shape) in &draft.shapes {
            let changed = self
                .shapes
                .shapes
                .get(id)
                .is_none_or(|old| !Arc::ptr_eq(&old.brep, &shape.brep));
            if changed {
                self.precision_guard(&shape.brep)?;
                let expected = self
                    .shapes
                    .high_water
                    .get(id)
                    .map_or(0, |value| value.saturating_add(1));
                if shape.revision != expected || shape.brep.revision != expected {
                    return Err(GraphError::code(
                        ErrorCode::RevisionConflict,
                        "reserved shape revision changed",
                    ));
                }
            }
        }
        Ok(())
    }

    fn add_node_changes(
        &self,
        draft: &Candidate,
        before_nodes: &mut BTreeMap<String, Option<Node>>,
        changes: &mut ChangeSet,
    ) {
        let node_keys: BTreeSet<_> = self
            .nodes
            .keys()
            .chain(draft.nodes.keys())
            .cloned()
            .collect();
        for id in node_keys {
            let old = self.nodes.get(&id);
            let new = draft.nodes.get(&id);
            if old != new {
                before_nodes.insert(id.clone(), old.cloned());
                match (old, new) {
                    (None, Some(node)) => changes.added.push(Self::change_for(node, &draft.shapes)),
                    (Some(node), None) => changes
                        .removed
                        .push(Self::change_for(node, &self.shapes.shapes)),
                    (Some(previous), Some(node)) if previous.generation != node.generation => {
                        changes
                            .removed
                            .push(Self::change_for(previous, &self.shapes.shapes));
                        changes.added.push(Self::change_for(node, &draft.shapes));
                    }
                    (Some(_), Some(node)) => {
                        changes.changed.push(Self::change_for(node, &draft.shapes))
                    }
                    (None, None) => {}
                }
            } else if draft.affected.contains(&id) {
                if let Some(node) = new {
                    changes.changed.push(Self::change_for(node, &draft.shapes));
                }
            }
        }
    }

    fn add_shape_changes(
        &self,
        draft: &Candidate,
        before_shapes: &mut BTreeMap<ShapeId, Option<Shape>>,
        changes: &mut ChangeSet,
    ) {
        let shape_keys: BTreeSet<_> = self
            .shapes
            .shapes
            .keys()
            .chain(draft.shapes.keys())
            .cloned()
            .collect();
        for id in shape_keys {
            let old = self.shapes.shapes.get(&id);
            let new = draft.shapes.get(&id);
            let changed = match (old, new) {
                (Some(a), Some(b)) => {
                    !Arc::ptr_eq(&a.brep, &b.brep)
                        || a.revision != b.revision
                        || a.users != b.users
                        || a.edge_keys != b.edge_keys
                        || a.report != b.report
                }
                (None, None) => false,
                _ => true,
            };
            if changed {
                before_shapes.insert(id.clone(), old.cloned());
                if let Some(shape) = new {
                    if old.is_none_or(|previous| previous.revision != shape.revision) {
                        for node in draft
                            .nodes
                            .values()
                            .filter(|node| node.shape.as_ref() == Some(&id))
                        {
                            let item = Self::change_for(node, &draft.shapes);
                            if !changes
                                .added
                                .iter()
                                .any(|change| change.og_id == item.og_id)
                                && !changes
                                    .changed
                                    .iter()
                                    .any(|change| change.og_id == item.og_id)
                            {
                                changes.changed.push(item);
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn mutate(
        &mut self,
        compute: impl FnOnce(&mut Candidate) -> Result<(), GraphError>,
    ) -> Result<ChangeSet, GraphError> {
        let mut draft = self.draft();
        compute(&mut draft)?;
        self.commit(draft)
    }
}
