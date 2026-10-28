use super::change_log::ChangeSet;
use super::error::{ErrorCode, ErrorContext, GraphError};
use super::graph::WorldGraph;
use super::modifying::validate_solid;
use super::node::EditScope;
use super::shape_store::ShapeId;
use crate::brep::{BodyType, GeometryError};
use crate::operations::modifying::boolean::{shell_brep, BooleanResult};

struct Shelled {
    result: BooleanResult,
    revision: u64,
    shape_id: ShapeId,
}

impl WorldGraph {
    pub fn shell(
        &mut self,
        og_id: &str,
        thickness: f64,
        scope: EditScope,
    ) -> Result<ChangeSet, GraphError> {
        let Shelled {
            result,
            revision,
            shape_id,
        } = self
            .shelled(og_id, thickness, scope)
            .map_err(|error| error.in_context(ErrorContext::Operate))?;
        self.replace_shape(&shape_id, result, Vec::new(), revision)
            .map_err(|error| error.in_context(ErrorContext::Operate))
    }

    fn shelled(
        &self,
        og_id: &str,
        thickness: f64,
        scope: EditScope,
    ) -> Result<Shelled, GraphError> {
        let target = self.ensure_editable(og_id, scope)?;
        if target.brep.body_type()? == BodyType::Sheet {
            return Err(GeometryError::CoverageGap {
                families: ["restricted analytic shell".into(), "open sheet".into()],
            }
            .into());
        }
        validate_solid(&target.brep)?;
        if !thickness.is_finite() || thickness <= 4.0 * target.brep.accuracy.geometric {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "shell thickness must be finite and above four times the geometric accuracy",
            ));
        }
        let shape_id = self.node(og_id)?.shape.clone().ok_or_else(|| {
            GraphError::code(ErrorCode::InvalidOperand, "shell target is not a body")
        })?;
        let revision = self.reserve(0, std::slice::from_ref(&shape_id))?.revisions[0].1;
        let mut result = shell_brep(&target.brep, thickness, shape_id.clone())?;
        result.brep.revision = revision;
        result.brep.validate()?;
        Ok(Shelled {
            result,
            revision,
            shape_id,
        })
    }
}
