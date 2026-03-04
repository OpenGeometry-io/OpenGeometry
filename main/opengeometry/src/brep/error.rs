use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BrepErrorKind {
    InvalidVertex,
    InvalidEdge,
    InvalidHalfEdge,
    InvalidLoop,
    InvalidWire,
    InvalidFace,
    InvalidShell,
    DegenerateEdge,
    DegenerateLoop,
    NonManifoldEdge,
    BrokenTopology,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrepError {
    pub kind: BrepErrorKind,
    pub message: String,
}

impl BrepError {
    pub fn new(kind: BrepErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl core::fmt::Display for BrepError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BrepError {}
