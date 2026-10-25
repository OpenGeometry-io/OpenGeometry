use super::candidate::IdKind;
use super::change_log::ChangeSet;
use super::error::{ErrorCode, ErrorContext, GraphError};
use super::graph::WorldGraph;
use super::node::EditScope;
use super::operands::{profile_loop, sweep_path};
use super::primitive::{check_body_type, check_no_plane, CreateOptions};
use crate::brep::{BodyType, BrepEnvelope, Similarity3};
use crate::operations::creating::{extrude, sweep};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum CreatingOperation {
    Extrude {
        profile: String,
        holes: Vec<String>,
        distance: f64,
    },
    Sweep {
        profile: String,
        path: String,
        #[serde(default)]
        caps: Option<bool>,
    },
}

impl CreatingOperation {
    fn anchor(&self) -> &str {
        match self {
            Self::Extrude { profile, .. } => profile,
            Self::Sweep { path, .. } => path,
        }
    }

    fn body_type(&self) -> BodyType {
        match self {
            Self::Sweep {
                caps: Some(false), ..
            } => BodyType::Sheet,
            _ => BodyType::Solid,
        }
    }
}

impl WorldGraph {
    fn build_operation(
        &self,
        id: String,
        operation: &CreatingOperation,
        target: &str,
    ) -> Result<BrepEnvelope, GraphError> {
        match operation {
            CreatingOperation::Extrude {
                profile,
                holes,
                distance,
            } => {
                let outer = profile_loop(self, profile, target)?;
                let holes = holes
                    .iter()
                    .map(|hole| {
                        if hole == profile {
                            return Err(GraphError::code(
                                ErrorCode::InvalidParameter,
                                "hole is the profile",
                            ));
                        }
                        profile_loop(self, hole, target)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                extrude::build(id, outer, holes, *distance, self.accuracy).map_err(GraphError::from)
            }
            CreatingOperation::Sweep {
                profile,
                path,
                caps,
            } => {
                let profile = profile_loop(self, profile, target)?;
                let path = sweep_path(self, path, target)?;
                sweep::build(id, profile, path, caps.unwrap_or(true), self.accuracy)
                    .map_err(GraphError::from)
            }
        }
    }

    pub fn create_operation(
        &mut self,
        operation: CreatingOperation,
        options: CreateOptions,
    ) -> Result<(String, ChangeSet), GraphError> {
        self.try_create_operation(operation, options)
            .map_err(|error| error.in_context(ErrorContext::Create))
    }

    fn try_create_operation(
        &mut self,
        operation: CreatingOperation,
        options: CreateOptions,
    ) -> Result<(String, ChangeSet), GraphError> {
        check_body_type(options.body_type, operation.body_type())?;
        check_no_plane(options.plane)?;
        let anchor = operation.anchor();
        let anchor_node = self.node(anchor)?;
        let parent = options.parent.or_else(|| anchor_node.parent.clone());
        if let Some(parent_id) = &parent {
            self.node(parent_id)?;
        }
        let local = if parent == anchor_node.parent {
            anchor_node.local
        } else {
            let world = self.world_placement(anchor)?;
            let parent_world = if let Some(parent_id) = &parent {
                self.world_placement(parent_id)?
            } else {
                Similarity3::IDENTITY
            };
            parent_world.inverse().compose(&world)
        };
        let reserved = self.reserve(1, &[])?;
        let shape_id = reserved.shape_ids[0].clone();
        let brep = self.build_operation(shape_id.clone(), &operation, anchor)?;
        let kind = IdKind::of_body(brep.body_type()?)?;
        let mut created = None;
        let changes = self.mutate(|draft| {
            let id = draft.new_og_id(options.og_id.as_deref(), kind)?;
            let allocated = draft.new_shape_id()?;
            if allocated != shape_id {
                return Err(GraphError::code(
                    ErrorCode::RevisionConflict,
                    "reserved shape id changed",
                ));
            }
            draft.add_body(&id, shape_id, brep, Vec::new(), &parent, local)?;
            created = Some(id);
            Ok(())
        })?;
        Ok((
            created.ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "created body is missing")
            })?,
            changes,
        ))
    }

    pub fn rebuild_operation(
        &mut self,
        og_id: &str,
        operation: CreatingOperation,
        scope: EditScope,
    ) -> Result<ChangeSet, GraphError> {
        self.try_rebuild_operation(og_id, operation, scope)
            .map_err(|error| error.in_context(ErrorContext::Rebuild))
    }

    fn try_rebuild_operation(
        &mut self,
        og_id: &str,
        operation: CreatingOperation,
        scope: EditScope,
    ) -> Result<ChangeSet, GraphError> {
        let previous = self.ensure_editable(og_id, scope)?;
        let body_type = previous.brep.body_type()?;
        if body_type == BodyType::Wire {
            return Err(GraphError::code(
                ErrorCode::BodyTypeMismatch,
                "body is not a solid",
            ));
        }
        let shape_id = self
            .node(og_id)?
            .shape
            .clone()
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidOperand, "node is not a body"))?;
        let revision = self.reserve(0, std::slice::from_ref(&shape_id))?.revisions[0].1;
        let mut brep = self.build_operation(shape_id.clone(), &operation, og_id)?;
        if brep.body_type()? != body_type {
            return Err(GraphError::code(
                ErrorCode::BodyTypeMismatch,
                "rebuild changed body type",
            ));
        }
        brep.revision = revision;
        brep.validate()?;
        self.mutate(|draft| {
            let shape = draft.shapes.get_mut(&shape_id).ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
            })?;
            shape.brep = Arc::new(brep);
            shape.revision = revision;
            shape.edge_keys.clear();
            shape.report = None;
            Ok(())
        })
    }
}
