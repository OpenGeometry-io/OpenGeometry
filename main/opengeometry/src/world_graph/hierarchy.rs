use super::candidate::Candidate;
use super::change_log::ChangeSet;
use super::error::{ErrorCode, GraphError};
use super::graph::WorldGraph;
use crate::brep::{BrepEnvelope, Similarity3};

impl Candidate {
    pub(super) fn world(&self, id: &str) -> Result<Similarity3, GraphError> {
        let mut path = Vec::new();
        let mut cursor = Some(id.to_string());
        while let Some(current) = cursor {
            let node = self
                .nodes
                .get(&current)
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, &current))?;
            cursor = node.parent.clone();
            path.push(node.local);
        }
        let mut world = Similarity3::IDENTITY;
        for local in path.iter().rev() {
            world = world.compose(local);
        }
        world.validate().map_err(|error| {
            GraphError::code(
                ErrorCode::InvalidTransform,
                format!("invalid world placement: {error}"),
            )
        })?;
        Ok(world)
    }

    fn check_parent(&self, child: &str, parent: &str) -> Result<(), GraphError> {
        let mut cursor = Some(parent.to_string());
        while let Some(current) = cursor {
            if current == child {
                return Err(GraphError::code(
                    ErrorCode::CycleDetected,
                    "reparent would create a cycle",
                ));
            }
            cursor = self
                .nodes
                .get(&current)
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, &current))?
                .parent
                .clone();
        }
        Ok(())
    }

    fn reparent(
        &mut self,
        child: &str,
        parent: Option<&str>,
        keep_world: bool,
    ) -> Result<(), GraphError> {
        if let Some(parent) = parent {
            self.check_parent(child, parent)?;
        }
        let previous = self
            .nodes
            .get(child)
            .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, child))?
            .parent
            .clone();
        let old_world = if keep_world {
            Some(self.world(child)?)
        } else {
            None
        };
        let parent_world = if keep_world {
            Some(match parent {
                Some(parent) => self.world(parent)?,
                None => Similarity3::IDENTITY,
            })
        } else {
            None
        };
        if previous.as_deref() == parent && !keep_world {
            return Ok(());
        }
        if let Some(previous) = &previous {
            let previous_node = self.nodes.get_mut(previous).ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "previous parent is missing")
            })?;
            previous_node.children.retain(|id| id != child);
        }
        if let Some(parent) = parent {
            let parent_node = self
                .nodes
                .get_mut(parent)
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, parent))?;
            parent_node.children.push(child.to_string());
        }
        let descendants = self.descendants(child)?;
        let node = self
            .nodes
            .get_mut(child)
            .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, child))?;
        node.parent = parent.map(str::to_string);
        if let (Some(old_world), Some(parent_world)) = (old_world, parent_world) {
            node.local = parent_world.inverse().compose(&old_world);
            node.local.validate().map_err(|error| {
                GraphError::code(
                    ErrorCode::InvalidTransform,
                    format!("invalid preserved world placement: {error}"),
                )
            })?;
        }
        self.affected.extend(descendants);
        Ok(())
    }
}

pub(super) fn restamp(
    mut brep: BrepEnvelope,
    previous: &str,
    next: &str,
) -> Result<BrepEnvelope, GraphError> {
    brep.id = next.into();
    brep.revision = 0;
    for face in &mut brep.topology.faces {
        for source in &mut face.provenance.sources {
            if source.entity == previous {
                source.entity = next.into();
            }
            if source.body == previous {
                source.body = next.into();
            }
        }
    }
    brep.validate()?;
    Ok(brep)
}

impl WorldGraph {
    pub fn children(&self, og_id: &str) -> Result<&[String], GraphError> {
        Ok(&self.node(og_id)?.children)
    }

    pub fn parent(&self, og_id: &str) -> Result<Option<&str>, GraphError> {
        Ok(self.node(og_id)?.parent.as_deref())
    }

    pub fn add_child(
        &mut self,
        parent: &str,
        children: &[String],
        keep_world: bool,
    ) -> Result<ChangeSet, GraphError> {
        self.mutate(|draft| {
            if !draft.nodes.contains_key(parent) {
                return Err(GraphError::code(ErrorCode::UnknownNode, parent));
            }
            for child in children {
                draft.reparent(child, Some(parent), keep_world)?;
            }
            Ok(())
        })
    }

    pub fn remove_child(
        &mut self,
        parent: &str,
        child: &str,
        keep_world: bool,
    ) -> Result<ChangeSet, GraphError> {
        self.mutate(|draft| {
            let actual = draft
                .nodes
                .get(child)
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, child))?
                .parent
                .as_deref();
            if actual != Some(parent) {
                return Err(GraphError::code(ErrorCode::NotAChild, child));
            }
            draft.reparent(child, None, keep_world)
        })
    }

    pub fn dispose(&mut self, og_id: &str) -> Result<ChangeSet, GraphError> {
        self.mutate(|draft| {
            let removed = draft.descendants(og_id)?;
            let parent = draft.nodes.get(og_id).and_then(|node| node.parent.clone());
            if let Some(parent) = parent {
                if let Some(parent_node) = draft.nodes.get_mut(&parent) {
                    parent_node.children.retain(|child| child != og_id);
                }
            }
            for id in removed {
                if let Some(node) = draft.nodes.remove(&id) {
                    if let Some(shape_id) = node.shape {
                        let shape = draft.shapes.get_mut(&shape_id).ok_or_else(|| {
                            GraphError::code(
                                ErrorCode::InvalidTopology,
                                "shape reference is missing",
                            )
                        })?;
                        shape.users -= 1;
                        if shape.users == 0 {
                            draft.shapes.remove(&shape_id);
                        }
                    }
                    draft.affected.insert(id);
                }
            }
            Ok(())
        })
    }
}
