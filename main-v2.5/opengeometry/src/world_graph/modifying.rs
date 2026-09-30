use super::change_log::ChangeSet;
use super::error::{ErrorCode, ErrorContext, ErrorDetails, GraphError};
use super::graph::WorldGraph;
use super::node::EditScope;
use super::placement::is_identity;
use super::shape_store::{Shape, ShapeId};
use crate::brep::{placed, BodyType, BrepEnvelope, GeometryError, GeometryQuality};
use crate::operations::modifying::boolean::{
    boolean_brep_outcome_with_handlers, subtract_planar_cutters_outcome_with_handlers, BooleanOp,
    BooleanResult,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ModifyingOperation {
    Union,
    Subtract,
    Intersect,
}

struct Operated {
    result: BooleanResult,
    handlers: Vec<String>,
    revision: u64,
    shape_id: ShapeId,
}

struct ToolFailure {
    tool_index: usize,
    error: GeometryError,
    handlers: Vec<String>,
}

impl WorldGraph {
    pub fn report(&self, og_id: &str) -> Result<Option<&serde_json::Value>, GraphError> {
        Ok(self.shape(og_id)?.report.as_ref())
    }

    pub fn operate(
        &mut self,
        target: &str,
        operation: ModifyingOperation,
        tools: &[String],
        scope: EditScope,
    ) -> Result<ChangeSet, GraphError> {
        let Operated {
            result,
            handlers,
            revision,
            shape_id,
        } = self
            .operated(target, operation, tools, scope)
            .map_err(|error| error.in_context(ErrorContext::Operate))?;
        let report = serde_json::json!({ "report": result.report, "handlers": handlers, "revision": revision });
        self.mutate(|draft| {
            let shape = draft.shapes.get_mut(&shape_id).ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "target shape is missing")
            })?;
            shape.brep = Arc::new(result.brep);
            shape.revision = revision;
            shape.edge_keys.clear();
            shape.report = Some(report);
            Ok(())
        })
        .map_err(|error| error.in_context(ErrorContext::Operate))
    }

    fn operated(
        &self,
        target: &str,
        operation: ModifyingOperation,
        tools: &[String],
        scope: EditScope,
    ) -> Result<Operated, GraphError> {
        if tools.is_empty() || tools.len() > 100 {
            return Err(GraphError::code(
                ErrorCode::LimitExceeded,
                "boolean requires 1..=100 tools",
            ));
        }
        let target_shape = self.ensure_editable(target, scope)?;
        validate_solid(&target_shape.brep)?;
        let shape_id =
            self.node(target)?.shape.clone().ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidOperand, "target is not a body")
            })?;
        let revision = self.reserve(0, std::slice::from_ref(&shape_id))?.revisions[0].1;
        let mapped = self.mapped_tools(target, tools, &shape_id)?;
        let (mut result, handlers) = kernel_result(target_shape, &mapped, operation, &shape_id)
            .map_err(|failure| failure.graph_error(target, tools))?
            .ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "boolean produced no result")
            })?;
        if result.brep.solids.is_empty() && result.brep.topology.faces.is_empty() {
            return Err(GraphError::code(
                ErrorCode::EmptyResult,
                "boolean result is empty",
            ));
        }
        validate_solid(&result.brep)?;
        result.brep.revision = revision;
        result.brep.validate()?;
        Ok(Operated {
            result,
            handlers,
            revision,
            shape_id,
        })
    }

    fn mapped_tools(
        &self,
        target: &str,
        tools: &[String],
        shape_id: &ShapeId,
    ) -> Result<Vec<BrepEnvelope>, GraphError> {
        let mut mapped = Vec::with_capacity(tools.len());
        for tool in tools {
            let tool_node = self.node(tool)?;
            if tool == target || tool_node.shape.as_ref() == Some(shape_id) {
                return Err(GraphError::code(
                    ErrorCode::ToolSharesTargetShape,
                    "tool shares target shape",
                ));
            }
            let shape = self.shape(tool)?;
            validate_solid(&shape.brep)?;
            let transform = self.relative_placement(tool, target)?;
            if transform.scale.to_bits() != 1.0f64.to_bits() {
                return Err(GraphError::code(
                    ErrorCode::InvalidOperand,
                    "tool has a relative scale",
                ));
            }
            mapped.push(if is_identity(transform) {
                (*shape.brep).clone()
            } else {
                placed(&shape.brep, transform.frame, 1.0)?
            });
        }
        Ok(mapped)
    }
}

fn validate_solid(body: &BrepEnvelope) -> Result<(), GraphError> {
    if body.body_type()? != BodyType::Solid {
        return Err(GraphError::code(
            ErrorCode::InvalidOperand,
            "boolean operand is not a solid",
        ));
    }
    if !matches!(body.quality, GeometryQuality::Analytic) {
        return Err(GraphError::code(
            ErrorCode::UnsupportedGeometry,
            "boolean operand is not analytic",
        ));
    }
    Ok(())
}

impl ToolFailure {
    fn graph_error(self, target: &str, tools: &[String]) -> GraphError {
        let source = GraphError::Geometry(self.error);
        GraphError::with_details(
            source.error_code(),
            source.message(),
            ErrorDetails::Operate {
                handlers: self.handlers,
                tool_index: self.tool_index,
                og_ids: std::iter::once(target.to_string())
                    .chain(tools.iter().cloned())
                    .collect(),
            },
        )
    }
}

fn kernel_result(
    target_shape: &Shape,
    mapped: &[BrepEnvelope],
    operation: ModifyingOperation,
    shape_id: &ShapeId,
) -> Result<Option<(BooleanResult, Vec<String>)>, ToolFailure> {
    let kernel_op = match operation {
        ModifyingOperation::Union => BooleanOp::Union,
        ModifyingOperation::Subtract => BooleanOp::Subtraction,
        ModifyingOperation::Intersect => BooleanOp::Intersection,
    };
    let (result, handlers) = if mapped.len() == 1 {
        boolean_brep_outcome_with_handlers(
            &target_shape.brep,
            &mapped[0],
            kernel_op,
            shape_id.clone(),
        )
    } else if operation == ModifyingOperation::Subtract {
        subtract_planar_cutters_outcome_with_handlers(&target_shape.brep, mapped, shape_id.clone())
    } else {
        return chained_boolean(target_shape, mapped, kernel_op, shape_id);
    };
    match result {
        Ok(result) => Ok(Some((result, handlers))),
        Err(error) => Err(ToolFailure {
            tool_index: 0,
            error,
            handlers,
        }),
    }
}

fn chained_boolean(
    target_shape: &Shape,
    mapped: &[BrepEnvelope],
    kernel_op: BooleanOp,
    shape_id: &ShapeId,
) -> Result<Option<(BooleanResult, Vec<String>)>, ToolFailure> {
    let mut handlers = Vec::new();
    let count = mapped.len();
    let mut final_result: Option<BooleanResult> = None;
    for (index, tool) in mapped.iter().enumerate() {
        let id = if index + 1 == count {
            shape_id.clone()
        } else {
            format!("{shape_id}:part:{index}")
        };
        let current = final_result
            .as_ref()
            .map_or(&*target_shape.brep, |result| &result.brep);
        let (result, step_handlers) =
            boolean_brep_outcome_with_handlers(current, tool, kernel_op, id);
        handlers.extend(step_handlers);
        match result {
            Ok(result) => final_result = Some(result),
            Err(error) => {
                return Err(ToolFailure {
                    tool_index: index,
                    error,
                    handlers,
                })
            }
        }
    }
    Ok(final_result.map(|result| (result, handlers)))
}
