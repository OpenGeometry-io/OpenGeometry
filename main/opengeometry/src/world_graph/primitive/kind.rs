use crate::brep::BodyType;
use crate::math::Point3;
use crate::primitives::{PolysolidJustify, PolysolidPath, ProfileSegment};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Primitive {
    Cuboid {
        width: f64,
        height: f64,
        depth: f64,
    },
    Cylinder {
        radius: f64,
        height: f64,
    },
    #[serde(rename_all = "camelCase")]
    CylinderSector {
        radius: f64,
        height: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
    Polyhedron {
        points: Vec<Point3>,
        faces: Vec<Vec<u32>>,
    },
    Sphere {
        radius: f64,
    },
    #[serde(rename_all = "camelCase")]
    Cone {
        radius_bottom: f64,
        radius_top: f64,
        height: f64,
    },
    #[serde(rename_all = "camelCase")]
    Torus {
        major_radius: f64,
        minor_radius: f64,
    },
    Rectangle {
        width: f64,
        breadth: f64,
    },
    Circle {
        radius: f64,
    },
    Polyline {
        points: Vec<Point3>,
        closed: bool,
    },
    Profile {
        start: [f64; 2],
        segments: Vec<ProfileSegment>,
        closed: bool,
    },
    #[serde(rename_all = "camelCase")]
    Arc {
        radius: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
    SlotPrism {
        length: f64,
        width: f64,
        height: f64,
    },
    RoundedBox {
        width: f64,
        height: f64,
        depth: f64,
        radius: f64,
    },
    #[serde(rename_all = "camelCase")]
    PipeSegment {
        outer_radius: f64,
        inner_radius: f64,
        height: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
    Polysolid {
        path: PolysolidPath,
        width: f64,
        height: f64,
        #[serde(default)]
        justify: PolysolidJustify,
    },
    RegularPolygon {
        sides: u32,
        radius: f64,
    },
    Slot {
        length: f64,
        width: f64,
    },
    RoundedRectangle {
        width: f64,
        breadth: f64,
        radius: f64,
    },
}

impl Primitive {
    pub(super) fn body_type(&self) -> BodyType {
        match self {
            Self::Cuboid { .. }
            | Self::Cylinder { .. }
            | Self::CylinderSector { .. }
            | Self::Polyhedron { .. }
            | Self::Sphere { .. }
            | Self::Cone { .. }
            | Self::Torus { .. }
            | Self::SlotPrism { .. }
            | Self::RoundedBox { .. }
            | Self::PipeSegment { .. }
            | Self::Polysolid { .. } => BodyType::Solid,
            Self::Rectangle { .. }
            | Self::Circle { .. }
            | Self::Polyline { .. }
            | Self::Profile { .. }
            | Self::Arc { .. }
            | Self::RegularPolygon { .. }
            | Self::Slot { .. }
            | Self::RoundedRectangle { .. } => BodyType::Wire,
        }
    }
}
