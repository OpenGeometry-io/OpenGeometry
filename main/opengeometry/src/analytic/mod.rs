pub mod booleans;
pub mod box_booleans;
pub mod diagnostics;
pub mod exchange;
pub mod export_curve;
pub mod face_intersection;
pub mod geometry;
pub mod ifc_exchange;
pub mod intersection;
pub mod modeling;
pub mod placement;
pub mod primitives;
pub mod query;
pub mod ssi;
pub mod tessellation;
pub mod topology;
pub mod universal_ssi;
pub mod wasm;

use crate::math::MathError;
use serde::{Deserialize, Serialize};
use std::fmt;

pub use geometry::{Curve, CurveGeometry, Frame3, Point3, Surface, SurfaceGeometry, UV};
pub use topology::{BrepEnvelope, GeometryStore};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
