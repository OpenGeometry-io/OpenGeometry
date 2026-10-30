use crate::world_graph::error::{ErrorCode, GraphError};

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
