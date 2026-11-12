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

pub(super) fn check_slot(length: f64, width: f64, g: f64) -> Result<(), GraphError> {
    required_size(length, g, "length")?;
    required_size(width / 2.0, g, "width")
}

pub(super) fn check_corner_radius(
    kind: &str,
    sides: [(&str, f64); 2],
    radius: f64,
    g: f64,
) -> Result<(), GraphError> {
    for (name, value) in sides.into_iter().chain([("radius", radius)]) {
        required_size(value, g, name)?;
    }
    if sides[0].1.min(sides[1].1) - 2.0 * radius <= 4.0 * g {
        return Err(parameter(&format!(
            "{kind} radius must be below half its {} and {}",
            sides[0].0, sides[1].0
        )));
    }
    Ok(())
}
