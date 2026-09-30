use crate::world_graph::{ErrorCode, GraphError};
use serde::de::DeserializeOwned;

pub(super) fn parse<T: DeserializeOwned>(json: &str) -> Result<T, GraphError> {
    if json.len() > 64 * 1024 {
        return Err(GraphError::code(
            ErrorCode::LimitExceeded,
            "params JSON exceeds 64 KiB",
        ));
    }
    serde_json::from_str(json).map_err(|error| {
        GraphError::code(
            ErrorCode::InvalidParameter,
            format!("invalid params JSON: {error}"),
        )
    })
}

pub(super) fn id(value: &str) -> Result<(), GraphError> {
    if value.chars().count() > 1024 {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "ogId exceeds 1024 characters",
        ));
    }
    Ok(())
}

pub(super) fn ids(values: &[String]) -> Result<(), GraphError> {
    for value in values {
        id(value)?;
    }
    Ok(())
}

pub(super) fn points(values: &[f64]) -> Result<Vec<[f64; 3]>, GraphError> {
    if values.len() % 3 != 0 || values.len() / 3 > 100_000 {
        return Err(GraphError::code(
            ErrorCode::LimitExceeded,
            "polyline point limit",
        ));
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "polyline point is nonfinite",
        ));
    }
    Ok(values
        .chunks_exact(3)
        .map(|point| [point[0], point[1], point[2]])
        .collect())
}
