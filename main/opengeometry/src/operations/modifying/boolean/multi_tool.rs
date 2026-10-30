use super::batch::subtract_planar_cutters_outcome_with_trace;
use super::dispatch::{boolean_brep_outcome_with_trace, Trace};
use super::types::{BooleanOp, BooleanResult};
use crate::brep::{BrepEnvelope, GeometryError};

pub struct MultiToolOutcome {
    pub result: Result<BooleanResult, GeometryError>,
    pub handlers: Vec<String>,
    pub attempted: Vec<String>,
    pub families: Option<[String; 2]>,
    pub tool_index: usize,
}

pub fn multi_tool_boolean(
    host: &BrepEnvelope,
    tools: &[BrepEnvelope],
    operation: BooleanOp,
    id: String,
) -> MultiToolOutcome {
    if tools.len() == 1 {
        let (result, trace) = boolean_brep_outcome_with_trace(host, &tools[0], operation, id);
        return outcome(result, trace, 0);
    }
    if operation != BooleanOp::Subtraction {
        let count = tools.len();
        return serial_outcome(host, tools, operation, Trace::default(), |index| {
            if index + 1 == count {
                id.clone()
            } else {
                format!("{id}:part:{index}")
            }
        });
    }
    let (result, trace) = subtract_planar_cutters_outcome_with_trace(host, tools, id.clone());
    match result {
        Err(GeometryError::CoverageGap { families })
            if !families[0].contains("mixed cutter batch") =>
        {
            serial_outcome(host, tools, operation, trace, |index| {
                format!("{id}:part:{index}")
            })
        }
        result => outcome(result, trace, 0),
    }
}

fn outcome(
    result: Result<BooleanResult, GeometryError>,
    trace: Trace,
    tool_index: usize,
) -> MultiToolOutcome {
    MultiToolOutcome {
        result,
        handlers: trace.handlers,
        attempted: trace.attempted,
        families: trace.families,
        tool_index,
    }
}

fn serial_outcome(
    host: &BrepEnvelope,
    tools: &[BrepEnvelope],
    operation: BooleanOp,
    mut trace: Trace,
    step_id: impl Fn(usize) -> String,
) -> MultiToolOutcome {
    let mut current: Option<BooleanResult> = None;
    for (index, tool) in tools.iter().enumerate() {
        let base = current.as_ref().map_or(host, |result| &result.brep);
        let (result, step) = boolean_brep_outcome_with_trace(base, tool, operation, step_id(index));
        trace.handlers.extend(step.handlers);
        trace.attempted.extend(step.attempted);
        trace.families = step.families;
        match result {
            Ok(result)
                if result.brep.solids.is_empty() && result.brep.topology.faces.is_empty() =>
            {
                current = Some(result);
                break;
            }
            Ok(result) => current = Some(result),
            Err(error) => return outcome(Err(error), trace, index),
        }
    }
    let result = current.ok_or_else(|| {
        GeometryError::InvalidGeometry("multi-tool boolean requires at least one tool".into())
    });
    outcome(result, trace, 0)
}
