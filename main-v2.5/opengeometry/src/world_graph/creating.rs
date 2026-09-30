use super::change_log::ChangeSet;
use super::error::{ErrorCode, GraphError};
use super::graph::WorldGraph;
use super::node::EditScope;
use super::operands::{path_points, profile_loop};
use super::primitive::CreateOptions;
use crate::brep::{BodyType, BrepEnvelope, Similarity3};
use crate::operations::creating::{extrude, sweep};
use crate::operations::CreatingOperation;
use std::sync::Arc;

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
                extrude::build(id, outer, holes, *distance, self.accuracy)
            }
            CreatingOperation::Sweep { profile, path } => {
                let profile = profile_loop(self, profile, target)?;
                let path = path_points(self, path, target)?;
                sweep::build(id, profile, path, self.accuracy)
            }
        }
    }

    pub fn create_operation(
        &mut self,
        operation: CreatingOperation,
        options: CreateOptions,
    ) -> Result<(String, ChangeSet), GraphError> {
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
        if brep.body_type()? != BodyType::Solid {
            return Err(GraphError::code(
                ErrorCode::BodyTypeMismatch,
                "operation did not produce a solid",
            ));
        }
        let mut created = None;
        let changes = self.mutate(|draft| {
            let id = draft.new_og_id(options.og_id.as_deref())?;
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
        let previous = self.ensure_editable(og_id, scope)?;
        if previous.brep.body_type()? != BodyType::Solid {
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
        if brep.body_type()? != BodyType::Solid {
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
