use crate::brep::GeometryError;

#[derive(Clone, Debug, PartialEq)]
pub enum OperationError {
    InvalidParameter(String),
    SweepSelfIntersection(String),
    InvalidTopology(String),
    Geometry(GeometryError),
}

impl From<GeometryError> for OperationError {
    fn from(value: GeometryError) -> Self {
        Self::Geometry(value)
    }
}

impl std::fmt::Display for OperationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OperationError {}
