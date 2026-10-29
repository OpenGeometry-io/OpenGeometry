use super::change_log::ChangeSet;
use super::error::{ErrorCode, ErrorContext, GraphError};
use super::graph::WorldGraph;
use super::node::EditScope;
use super::shape_store::ShapeId;
use crate::brep::{mirrored, BrepEnvelope};
use crate::math::Point3;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MirrorPlane {
    pub origin: Point3,
    pub normal: Point3,
}

impl WorldGraph {
    pub fn mirror(
        &mut self,
        og_id: &str,
        plane: MirrorPlane,
        scope: EditScope,
    ) -> Result<ChangeSet, GraphError> {
        let (shape_id, brep) = self
            .mirrored_shape(og_id, plane, scope)
            .map_err(|error| error.in_context(ErrorContext::Operate))?;
        self.mutate(|draft| {
            let shape = draft.shapes.get_mut(&shape_id).ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "target shape is missing")
            })?;
            shape.revision = brep.revision;
            shape.brep = Arc::new(brep);
            shape.report = None;
            Ok(())
        })
        .map_err(|error| error.in_context(ErrorContext::Operate))
    }

    fn mirrored_shape(
        &self,
        og_id: &str,
        plane: MirrorPlane,
        scope: EditScope,
    ) -> Result<(ShapeId, BrepEnvelope), GraphError> {
        let target = self.ensure_editable(og_id, scope)?;
        check_plane(plane)?;
        let shape_id = self.node(og_id)?.shape.clone().ok_or_else(|| {
            GraphError::code(ErrorCode::InvalidOperand, "mirror target is not a body")
        })?;
        let revision = self.reserve(0, std::slice::from_ref(&shape_id))?.revisions[0].1;
        let mut brep = mirrored(&target.brep, plane.origin, plane.normal)?;
        brep.revision = revision;
        Ok((shape_id, brep))
    }
}

fn check_plane(plane: MirrorPlane) -> Result<(), GraphError> {
    let finite = plane
        .origin
        .iter()
        .chain(&plane.normal)
        .all(|v| v.is_finite());
    if !finite || plane.normal.iter().all(|v| *v == 0.0) {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "mirror plane needs a finite origin and a finite nonzero normal",
        ));
    }
    Ok(())
}
