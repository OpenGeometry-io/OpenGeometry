use crate::brep::{BrepEnvelope, FaceSource, GeometryQuality};
use crate::math::Point3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOp {
    Union,
    Intersection,
    Subtraction,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct FaceMapping {
    pub(super) source: FaceSource,
    pub(super) result_faces: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BooleanReport {
    pub(super) operation: BooleanOp,
    pub(super) quality: GeometryQuality,
    pub(super) contacts: Vec<Point3>,
    pub(super) coincident: bool,
    pub(super) face_mappings: Vec<FaceMapping>,
}

pub struct BooleanResult {
    pub brep: BrepEnvelope,
    pub report: BooleanReport,
}
