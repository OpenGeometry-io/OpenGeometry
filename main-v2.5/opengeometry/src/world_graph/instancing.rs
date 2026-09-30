use super::candidate::Candidate;
use super::change_log::ChangeSet;
use super::error::{ErrorCode, GraphError};
use super::graph::WorldGraph;
use super::hierarchy::restamp;
use super::node::Node;
use super::shape_store::Shape;
use crate::brep::Similarity3;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CopyOptions {
    pub og_id: Option<String>,
    pub parent: Option<Option<String>>,
}

impl WorldGraph {
    pub fn instance(
        &mut self,
        source: &str,
        options: CopyOptions,
    ) -> Result<(String, ChangeSet), GraphError> {
        let mut created = None;
        let changes = self.mutate(|draft| {
            let original = draft
                .nodes
                .get(source)
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, source))?
                .clone();
            let shape_id = original.shape.clone().ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidOperand, "an assembly cannot be instanced")
            })?;
            let (parent, local) = copy_placement(
                draft,
                source,
                &options,
                &original,
                "invalid instance placement",
            )?;
            let id = draft.new_og_id(options.og_id.as_deref())?;
            let handle = draft.new_handle()?;
            let shape = draft.shapes.get_mut(&shape_id).ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
            })?;
            shape.users = shape.users.checked_add(1).ok_or_else(|| {
                GraphError::code(ErrorCode::LimitExceeded, "instance count overflow")
            })?;
            draft.add_node(&id, handle, &parent, local, Some(shape_id), original.kind);
            created = Some(id);
            Ok(())
        })?;
        Ok((
            created.ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "created instance is missing")
            })?,
            changes,
        ))
    }

    pub fn duplicate(
        &mut self,
        source: &str,
        options: CopyOptions,
    ) -> Result<(String, ChangeSet), GraphError> {
        let mut created = None;
        let changes = self.mutate(|draft| {
            let original = draft
                .nodes
                .get(source)
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, source))?
                .clone();
            let old_shape_id = original.shape.as_ref().ok_or_else(|| {
                GraphError::code(
                    ErrorCode::InvalidOperand,
                    "an assembly cannot be duplicated",
                )
            })?;
            let old_shape = draft
                .shapes
                .get(old_shape_id)
                .ok_or_else(|| {
                    GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
                })?
                .clone();
            let (parent, local) = copy_placement(
                draft,
                source,
                &options,
                &original,
                "invalid duplicate placement",
            )?;
            let id = draft.new_og_id(options.og_id.as_deref())?;
            let shape_id = draft.new_shape_id()?;
            let brep = restamp((*old_shape.brep).clone(), old_shape_id, &shape_id)?;
            let handle = draft.new_handle()?;
            draft.shapes.insert(
                shape_id.clone(),
                Shape {
                    brep: Arc::new(brep),
                    revision: 0,
                    users: 1,
                    edge_keys: old_shape.edge_keys,
                    report: old_shape.report,
                },
            );
            draft.add_node(&id, handle, &parent, local, Some(shape_id), original.kind);
            created = Some(id);
            Ok(())
        })?;
        Ok((
            created.ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "created duplicate is missing")
            })?,
            changes,
        ))
    }

    pub fn make_unique(&mut self, og_id: &str) -> Result<Option<ChangeSet>, GraphError> {
        if self.shape(og_id)?.users == 1 {
            return Ok(None);
        }
        self.mutate(|draft| {
            let old_shape_id = draft
                .nodes
                .get(og_id)
                .and_then(|node| node.shape.clone())
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, og_id))?;
            let old_shape = draft
                .shapes
                .get(&old_shape_id)
                .ok_or_else(|| {
                    GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
                })?
                .clone();
            let new_shape_id = draft.new_shape_id()?;
            let brep = restamp((*old_shape.brep).clone(), &old_shape_id, &new_shape_id)?;
            draft.shapes.insert(
                new_shape_id.clone(),
                Shape {
                    brep: Arc::new(brep),
                    revision: 0,
                    users: 1,
                    edge_keys: old_shape.edge_keys,
                    report: old_shape.report,
                },
            );
            draft
                .shapes
                .get_mut(&old_shape_id)
                .ok_or_else(|| {
                    GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
                })?
                .users -= 1;
            draft
                .nodes
                .get_mut(og_id)
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, og_id))?
                .shape = Some(new_shape_id);
            draft.affected.insert(og_id.into());
            Ok(())
        })
        .map(Some)
    }
}

fn copy_placement(
    draft: &Candidate,
    source: &str,
    options: &CopyOptions,
    original: &Node,
    message: &str,
) -> Result<(Option<String>, Similarity3), GraphError> {
    let parent = options
        .parent
        .clone()
        .unwrap_or_else(|| original.parent.clone());
    if let Some(parent) = &parent {
        if !draft.nodes.contains_key(parent) {
            return Err(GraphError::code(ErrorCode::UnknownNode, parent));
        }
    }
    let local = if parent != original.parent {
        let world = draft.world(source)?;
        let parent_world = match &parent {
            Some(parent) => draft.world(parent)?,
            None => Similarity3::IDENTITY,
        };
        parent_world.inverse().compose(&world)
    } else {
        original.local
    };
    local.validate().map_err(|error| {
        GraphError::code(ErrorCode::InvalidTransform, format!("{message}: {error}"))
    })?;
    Ok((parent, local))
}
