use crate::brep::GeometryError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ErrorCode {
    NotInitialised,
    InvalidParameter,
    InvalidTransform,
    InvalidOperand,
    UnknownNode,
    DuplicateNode,
    CycleDetected,
    NotAChild,
    Disposed,
    BodyTypeMismatch,
    SharedShape,
    ToolSharesTargetShape,
    EmptyResult,
    SweepSelfIntersection,
    CoverageGap,
    UnresolvedIntersection,
    UnresolvedTessellation,
    UnsupportedGeometry,
    LimitExceeded,
    InvalidGeometry,
    InvalidTopology,
    InvalidMark,
    WorkerFailure,
    KernelPanic,
    RevisionConflict,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum GraphError {
    Code {
        code: ErrorCode,
        message: String,
        details: Value,
    },
    Geometry(GeometryError),
}

impl GraphError {
    pub(crate) fn code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Code {
            code,
            message: message.into(),
            details: Value::Null,
        }
    }

    pub(super) fn with_details(
        code: ErrorCode,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self::Code {
            code,
            message: message.into(),
            details,
        }
    }

    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::Code { code, .. } => *code,
            Self::Geometry(error) => match error {
                GeometryError::Math(_) | GeometryError::InvalidGeometry(_) => {
                    ErrorCode::InvalidGeometry
                }
                GeometryError::InvalidTopology(_)
                | GeometryError::MissingReference { .. }
                | GeometryError::UnsupportedSchema { .. } => ErrorCode::InvalidTopology,
                GeometryError::UnsupportedGeometry(_) => ErrorCode::UnsupportedGeometry,
                GeometryError::AmbiguousProfileAlignment(_)
                | GeometryError::SingularParameterization => ErrorCode::InvalidParameter,
                GeometryError::CoverageGap { .. } => ErrorCode::CoverageGap,
                GeometryError::UnresolvedIntersection(_) => ErrorCode::UnresolvedIntersection,
                GeometryError::UnresolvedTessellation(_) => ErrorCode::UnresolvedTessellation,
                GeometryError::LimitExceeded(_) => ErrorCode::LimitExceeded,
            },
        }
    }
}

impl From<GeometryError> for GraphError {
    fn from(value: GeometryError) -> Self {
        Self::Geometry(value)
    }
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GraphError {}
