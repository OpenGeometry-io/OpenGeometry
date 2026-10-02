use crate::brep::GeometryError;
use crate::operations::OperationError;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorContext {
    Create,
    Rebuild,
    Transform,
    Operate,
    Export,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub enum ErrorDetails {
    None,
    SharedShape {
        shape_id: Option<String>,
        instance_count: u32,
        sharing: Vec<String>,
    },
    MissingReference {
        kind: String,
        index: u32,
    },
    CoverageGap {
        families: [String; 2],
    },
    Math {
        math: String,
    },
    Operate {
        handlers: Vec<String>,
        tool_index: usize,
        og_ids: Vec<String>,
    },
    Export {
        og_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum GraphError {
    Code {
        code: ErrorCode,
        message: String,
        details: ErrorDetails,
    },
    Geometry(GeometryError),
}

impl GraphError {
    pub(crate) fn code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Code {
            code,
            message: message.into(),
            details: ErrorDetails::None,
        }
    }

    pub(super) fn with_details(
        code: ErrorCode,
        message: impl Into<String>,
        details: ErrorDetails,
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
                GeometryError::AmbiguousProfileAlignment(_) => ErrorCode::InvalidParameter,
                GeometryError::SingularParameterization => ErrorCode::InvalidGeometry,
                GeometryError::CoverageGap { .. } => ErrorCode::CoverageGap,
                GeometryError::UnresolvedIntersection(_) => ErrorCode::UnresolvedIntersection,
                GeometryError::UnresolvedTessellation(_) => ErrorCode::UnresolvedTessellation,
                GeometryError::LimitExceeded(_) => ErrorCode::LimitExceeded,
            },
        }
    }

    pub fn details(&self) -> ErrorDetails {
        match self {
            Self::Code { details, .. } => details.clone(),
            Self::Geometry(GeometryError::MissingReference { kind, index }) => {
                ErrorDetails::MissingReference {
                    kind: kind.clone(),
                    index: *index,
                }
            }
            Self::Geometry(GeometryError::CoverageGap { families }) => ErrorDetails::CoverageGap {
                families: families.clone(),
            },
            Self::Geometry(GeometryError::Math(math)) => ErrorDetails::Math {
                math: format!("{math:?}"),
            },
            Self::Geometry(_) => ErrorDetails::None,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Code { message, .. } => message.clone(),
            Self::Geometry(error) => error.to_string(),
        }
    }

    pub fn in_context(self, context: ErrorContext) -> GraphError {
        if self != Self::Geometry(GeometryError::SingularParameterization) {
            return self;
        }
        match context {
            ErrorContext::Create | ErrorContext::Rebuild => {
                Self::code(ErrorCode::InvalidParameter, "SingularParameterization")
            }
            ErrorContext::Transform => {
                Self::code(ErrorCode::InvalidTransform, "SingularParameterization")
            }
            ErrorContext::Operate | ErrorContext::Export => self,
        }
    }
}

impl From<GeometryError> for GraphError {
    fn from(value: GeometryError) -> Self {
        Self::Geometry(value)
    }
}

impl From<OperationError> for GraphError {
    fn from(value: OperationError) -> Self {
        match value {
            OperationError::InvalidParameter(message) => {
                Self::code(ErrorCode::InvalidParameter, message)
            }
            OperationError::SweepSelfIntersection(message) => {
                Self::code(ErrorCode::SweepSelfIntersection, message)
            }
            OperationError::InvalidTopology(message) => {
                Self::code(ErrorCode::InvalidTopology, message)
            }
            OperationError::Geometry(error) => Self::Geometry(error),
        }
    }
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GraphError {}
