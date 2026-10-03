use super::batch::subtract_planar_cutters_outcome_with_handlers;
use super::dispatch::boolean_brep_outcome_with_handlers;
use super::types::{BooleanOp, BooleanResult};
use crate::brep::{BrepEnvelope, GeometryError};

pub struct MultiToolOutcome {
    pub result: Result<BooleanResult, GeometryError>,
    pub handlers: Vec<String>,
    pub tool_index: usize,
}

pub fn multi_tool_boolean(
    host: &BrepEnvelope,
    tools: &[BrepEnvelope],
    operation: BooleanOp,
    id: String,
) -> MultiToolOutcome {
    if tools.len() == 1 {
        let (result, handlers) = boolean_brep_outcome_with_handlers(host, &tools[0], operation, id);
        return MultiToolOutcome {
            result,
            handlers,
            tool_index: 0,
        };
    }
    if operation != BooleanOp::Subtraction {
        let count = tools.len();
        return serial_outcome(host, tools, operation, |index| {
            if index + 1 == count {
                id.clone()
            } else {
                format!("{id}:part:{index}")
            }
        });
    }
    let (result, handlers) = subtract_planar_cutters_outcome_with_handlers(host, tools, id.clone());
    match result {
        Err(GeometryError::CoverageGap { families })
            if !families[0].contains("mixed cutter batch") =>
        {
            serial_outcome(host, tools, operation, |index| format!("{id}:part:{index}"))
        }
        result => MultiToolOutcome {
            result,
            handlers,
            tool_index: 0,
        },
    }
}

fn serial_outcome(
    host: &BrepEnvelope,
    tools: &[BrepEnvelope],
    operation: BooleanOp,
    step_id: impl Fn(usize) -> String,
) -> MultiToolOutcome {
    let mut handlers = Vec::new();
    let mut current: Option<BooleanResult> = None;
    for (index, tool) in tools.iter().enumerate() {
        let base = current.as_ref().map_or(host, |result| &result.brep);
        let (result, step_handlers) =
            boolean_brep_outcome_with_handlers(base, tool, operation, step_id(index));
        handlers.extend(step_handlers);
        match result {
            Ok(result)
                if result.brep.solids.is_empty() && result.brep.topology.faces.is_empty() =>
            {
                current = Some(result);
                break;
            }
            Ok(result) => current = Some(result),
            Err(error) => {
                return MultiToolOutcome {
                    result: Err(error),
                    handlers,
                    tool_index: index,
                }
            }
        }
    }
    MultiToolOutcome {
        result: current.ok_or_else(|| {
            GeometryError::InvalidGeometry("multi-tool boolean requires at least one tool".into())
        }),
        handlers,
        tool_index: 0,
    }
}
