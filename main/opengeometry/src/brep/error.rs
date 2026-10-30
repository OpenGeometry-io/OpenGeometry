use crate::math::MathError;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum GeometryError {
    Math(MathError),
    InvalidGeometry(String),
    InvalidTopology(String),
    UnsupportedGeometry(String),
    AmbiguousProfileAlignment(String),
    CoverageGap { families: [String; 2] },
    SingularParameterization,
    MissingReference { kind: String, index: u32 },
    UnsupportedSchema { found: u32 },
    UnresolvedIntersection(String),
    UnresolvedTessellation(String),
    LimitExceeded(String),
}

impl From<MathError> for GeometryError {
    fn from(error: MathError) -> Self {
        Self::Math(error)
    }
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for GeometryError {}

pub(super) fn missing(kind: &str, index: u32) -> GeometryError {
    GeometryError::MissingReference {
        kind: kind.into(),
        index,
    }
}
pub(super) fn invalid(message: impl Into<String>) -> GeometryError {
    GeometryError::InvalidTopology(message.into())
}
pub(super) fn positive(value: f64) -> Result<(), GeometryError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "entity tolerance must be finite and positive".into(),
        ));
    }
    Ok(())
}
pub(crate) fn coverage_gap(a: &str, b: &str) -> GeometryError {
    GeometryError::CoverageGap {
        families: [a.into(), b.into()],
    }
}
