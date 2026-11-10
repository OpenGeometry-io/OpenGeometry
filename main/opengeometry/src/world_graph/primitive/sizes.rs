use crate::brep::Accuracy;
use crate::world_graph::error::{ErrorCode, GraphError};
use std::f64::consts::TAU;

pub(super) fn required_size(value: f64, accuracy: f64, name: &str) -> Result<(), GraphError> {
    if !value.is_finite() || value <= 4.0 * accuracy {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            format!("{name} must exceed four times geometric tolerance"),
        ));
    }
    Ok(())
}

pub(super) fn parameter(message: &str) -> GraphError {
    GraphError::code(ErrorCode::InvalidParameter, message)
}

pub(super) fn wrong_body_type(expected: &str) -> GraphError {
    GraphError::code(
        ErrorCode::BodyTypeMismatch,
        format!("primitive is not a {expected}"),
    )
}

pub(super) fn check_arc(
    kind: &str,
    radius: f64,
    [start, sweep]: [f64; 2],
    accuracy: Accuracy,
) -> Result<(), GraphError> {
    required_size(radius, accuracy.geometric, "radius")?;
    if !start.is_finite() || !sweep.is_finite() || sweep == 0.0 || sweep.abs() >= TAU {
        return Err(parameter(&format!(
            "{kind} sweep must be nonzero and less than one turn"
        )));
    }
    if radius * sweep.abs() <= 4.0 * accuracy.geometric {
        return Err(parameter(&format!(
            "{kind} length must exceed four times geometric tolerance"
        )));
    }
    Ok(())
}
