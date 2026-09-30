pub(super) mod creating;
pub mod modifying;

use crate::world_graph::{ErrorCode, GraphError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ModifyingOperation {
    Union,
    Subtract,
    Intersect,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum CreatingOperation {
    Extrude {
        profile: String,
        holes: Vec<String>,
        distance: f64,
    },
    Sweep {
        profile: String,
        path: String,
    },
}

impl CreatingOperation {
    pub(super) fn anchor(&self) -> &str {
        match self {
            Self::Extrude { profile, .. } => profile,
            Self::Sweep { path, .. } => path,
        }
    }
}

pub(super) fn invalid(message: impl Into<String>) -> GraphError {
    GraphError::code(ErrorCode::InvalidParameter, message)
}
