use super::frame::UV;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(crate) enum PcurveGeometry {
    Line2 {
        origin: UV,
        direction: UV,
    },
    Conic2 {
        origin: UV,
        axis_a: UV,
        axis_b: UV,
    },
    ProjectedCurve {
        curve: u32,
        surface: u32,
        chart: u32,
        uv_hint: UV,
        uv_rate: UV,
        parameter_origin: f64,
    },
    IntersectionSide {
        definition: u32,
        side: IntersectionSide,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum IntersectionSide {
    A,
    B,
}

pub(crate) fn period_lifted_uv(mut uv: UV, periods: [Option<f64>; 2], lift: [i32; 2]) -> UV {
    for axis in 0..2 {
        uv[axis] += periods[axis].map_or(0.0, |period| period * f64::from(lift[axis]));
    }
    uv
}
