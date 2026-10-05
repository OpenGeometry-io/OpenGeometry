use crate::brep::frame::UV;
use crate::math::{Interval, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TraceAnchor {
    pub(crate) parameter: f64,
    pub(crate) point: Point3,
    pub(crate) uv_a: UV,
    pub(crate) uv_b: UV,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IntersectionDefinition {
    pub(crate) surfaces: [u32; 2],
    pub(crate) anchors: Vec<TraceAnchor>,
    pub(crate) uv_tubes: Vec<[Interval; 4]>,
    pub(crate) residual_tolerance: f64,
}
