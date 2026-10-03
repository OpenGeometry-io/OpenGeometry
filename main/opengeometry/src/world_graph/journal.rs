use super::change_log::{ChangeSet, NodeChange};
use super::error::{ErrorCode, GraphError};
use super::graph::{LiveMark, WorldGraph};
use super::node::{MarkId, Node};
use super::shape_store::{Shape, ShapeId};
use super::tree_walk::descendants;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkStats {
    pub live_marks: usize,
    pub retained_revisions: usize,
}

impl WorldGraph {
    pub fn mark(&mut self) -> Result<MarkId, GraphError> {
        let id = MarkId(self.next_mark);
        self.next_mark = self
            .next_mark
            .checked_add(1)
            .ok_or_else(|| GraphError::code(ErrorCode::LimitExceeded, "mark counter overflow"))?;
        self.marks.push(LiveMark {
            id,
            position: self.journal.len(),
        });
        Ok(id)
    }

    pub fn mark_stats(&self) -> MarkStats {
        let mut retained = HashSet::new();
        for entry in &self.journal {
            for shape in entry.shapes.values().flatten() {
                retained.insert(Arc::as_ptr(&shape.brep) as usize);
            }
        }
        MarkStats {
            live_marks: self.marks.len(),
            retained_revisions: retained.len(),
        }
    }

    pub fn release(&mut self, mark: MarkId) -> Result<(), GraphError> {
        let index = self
            .marks
            .iter()
            .position(|item| item.id == mark)
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidMark, "mark is not live"))?;
        self.marks.remove(index);
        if let Some(earliest) = self.marks.iter().map(|item| item.position).min() {
            if earliest > 0 {
                self.journal.drain(..earliest);
                for item in &mut self.marks {
                    item.position -= earliest;
                }
            }
        } else {
            self.journal.clear();
        }
        Ok(())
    }

    pub fn rollback(&mut self, mark: MarkId) -> Result<ChangeSet, GraphError> {
        let index = self
            .marks
            .iter()
            .position(|item| item.id == mark)
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidMark, "mark is not live"))?;
        let position = self.marks[index].position;
        if position > self.journal.len() {
            return Err(GraphError::code(
                ErrorCode::InvalidMark,
                "journal position is invalid",
            ));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| GraphError::code(ErrorCode::LimitExceeded, "graph revision overflow"))?;
        let old_nodes = self.nodes.clone();
        let old_shapes = self.shapes.shapes.clone();
        self.unwind_journal(position)?;
        self.marks.truncate(index + 1);
        self.world_cache.borrow_mut().clear();
        let changes = self.rollback_changes(revision, &old_nodes, &old_shapes);
        self.revision = revision;
        self.changes.push(changes.clone());
        Ok(changes)
    }

    fn unwind_journal(&mut self, position: usize) -> Result<(), GraphError> {
        while self.journal.len() > position {
            let entry = self.journal.pop().ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidMark, "journal position is invalid")
            })?;
            for (id, before) in entry.nodes {
                match before {
                    Some(node) => {
                        self.nodes.insert(id, node);
                    }
                    None => {
                        self.nodes.remove(&id);
                    }
                }
            }
            for (id, before) in entry.shapes {
                match before {
                    Some(shape) => {
                        self.shapes.shapes.insert(id, shape);
                    }
                    None => {
                        self.shapes.shapes.remove(&id);
                    }
                }
            }
        }
        Ok(())
    }

    fn rollback_changes(
        &self,
        revision: u64,
        old_nodes: &BTreeMap<String, Node>,
        old_shapes: &BTreeMap<ShapeId, Shape>,
    ) -> ChangeSet {
        let mut changes = ChangeSet {
            revision,
            added: Vec::new(),
            changed: Vec::new(),
            removed: Vec::new(),
        };
        let mut affected = BTreeSet::new();
        let ids: BTreeSet<_> = old_nodes.keys().chain(self.nodes.keys()).cloned().collect();
        for id in ids {
            let before = old_nodes.get(&id);
            let after = self.nodes.get(&id);
            if before != after {
                affected.extend(descendants(old_nodes, &id));
                affected.extend(descendants(&self.nodes, &id));
                match (before, after) {
                    (None, Some(node)) => changes
                        .added
                        .push(Self::change_for(node, &self.shapes.shapes)),
                    (Some(node), None) => changes.removed.push(Self::change_for(node, old_shapes)),
                    (Some(old), Some(new)) if old.generation != new.generation => {
                        changes.removed.push(Self::change_for(old, old_shapes));
                        changes
                            .added
                            .push(Self::change_for(new, &self.shapes.shapes));
                    }
                    (Some(_), Some(node)) => changes
                        .changed
                        .push(Self::change_for(node, &self.shapes.shapes)),
                    (None, None) => {}
                }
            }
        }
        let shape_ids: BTreeSet<ShapeId> = old_shapes
            .keys()
            .chain(self.shapes.shapes.keys())
            .cloned()
            .collect();
        for id in shape_ids {
            let old_revision = old_shapes.get(&id).map(|shape| shape.revision);
            let new_revision = self.shapes.shapes.get(&id).map(|shape| shape.revision);
            if old_revision != new_revision {
                affected.extend(
                    self.nodes
                        .values()
                        .filter(|node| node.shape.as_ref() == Some(&id))
                        .map(|node| node.og_id.clone()),
                );
            }
        }
        for id in affected {
            if changes.added.iter().any(|item| item.og_id == id)
                || changes.removed.iter().any(|item| item.og_id == id)
                || changes.changed.iter().any(|item| item.og_id == id)
            {
                continue;
            }
            if let Some(node) = self.nodes.get(&id) {
                changes
                    .changed
                    .push(Self::change_for(node, &self.shapes.shapes));
            }
        }
        changes
    }

    pub fn changes_since(&self, revision: u64) -> Result<ChangeSet, GraphError> {
        if revision > self.revision {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "revision is in the future",
            ));
        }
        #[derive(Clone, Copy)]
        enum Kind {
            Added,
            Changed,
            Removed,
        }
        let mut compact: BTreeMap<(String, u32, u32), (Kind, NodeChange)> = BTreeMap::new();
        for change_set in self
            .changes
            .iter()
            .filter(|change| change.revision > revision)
        {
            for (kind, entries) in [
                (Kind::Added, &change_set.added),
                (Kind::Changed, &change_set.changed),
                (Kind::Removed, &change_set.removed),
            ] {
                for entry in entries {
                    let key = (entry.og_id.clone(), entry.handle, entry.generation);
                    match (compact.get(&key).map(|(kind, _)| *kind), kind) {
                        (Some(Kind::Added), Kind::Removed) => {
                            compact.remove(&key);
                        }
                        (Some(Kind::Added), Kind::Changed) => {
                            compact.insert(key, (Kind::Added, entry.clone()));
                        }
                        (Some(Kind::Removed), Kind::Added) => {
                            compact.insert(key, (Kind::Changed, entry.clone()));
                        }
                        (Some(Kind::Changed), Kind::Removed) => {
                            compact.insert(key, (Kind::Removed, entry.clone()));
                        }
                        _ => {
                            compact.insert(key, (kind, entry.clone()));
                        }
                    }
                }
            }
        }
        let mut result = ChangeSet {
            revision: self.revision,
            added: Vec::new(),
            changed: Vec::new(),
            removed: Vec::new(),
        };
        for (_, (kind, entry)) in compact {
            match kind {
                Kind::Added => result.added.push(entry),
                Kind::Changed => result.changed.push(entry),
                Kind::Removed => result.removed.push(entry),
            }
        }
        Ok(result)
    }
}
