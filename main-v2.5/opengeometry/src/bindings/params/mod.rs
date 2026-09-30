use crate::world_graph::{ErrorCode, GraphError};
use serde::de::{self, DeserializeOwned, Deserializer as _, IgnoredAny, SeqAccess, Visitor};
use std::fmt;

#[cfg(test)]
mod tests;

struct NodeFault<'a> {
    fault: &'a mut Option<GraphError>,
}

impl<'de> Visitor<'de> for NodeFault<'_> {
    type Value = Vec<String>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an array of node ids")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
        let mut values = Vec::new();
        while values.len() < 100_000 {
            let Some(value) = seq.next_element::<String>()? else {
                return Ok(values);
            };
            if let Err(error) = id(&value) {
                *self.fault = Some(error);
                return Err(de::Error::custom("ogId exceeds 1024 characters"));
            }
            values.push(value);
        }
        if seq.next_element::<IgnoredAny>()?.is_some() {
            *self.fault = Some(GraphError::code(
                ErrorCode::LimitExceeded,
                "exportStep nodes exceed 100,000 ids",
            ));
            return Err(de::Error::custom("exportStep nodes exceed 100,000 ids"));
        }
        Ok(values)
    }
}

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

pub(super) fn nodes(json: &str) -> Result<Vec<String>, GraphError> {
    let mut fault = None;
    let mut deserializer = serde_json::Deserializer::from_str(json);
    let values = deserializer
        .deserialize_seq(NodeFault { fault: &mut fault })
        .and_then(|values| deserializer.end().map(|()| values));
    values.map_err(|error| {
        fault.unwrap_or_else(|| {
            GraphError::code(
                ErrorCode::InvalidParameter,
                format!("invalid params JSON: {error}"),
            )
        })
    })
}
