use super::orientation::Orientation;
use super::provenance::FaceProvenance;
use crate::brep::frame::UVBox;
use crate::math::{Interval, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(crate) enum EdgeGeometry {
    Curve { curve: u32, range: Interval },
    Collapsed { vertex: u32 },
}
impl EdgeGeometry {
    pub(crate) fn range(&self) -> Interval {
        match self {
            Self::Curve { range, .. } => *range,
            Self::Collapsed { .. } => Interval { lo: 0.0, hi: 1.0 },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vertex {
    pub(crate) id: u32,
    pub position: Point3,
    pub(crate) outgoing_halfedge: Option<u32>,
    pub(crate) tolerance: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub(crate) id: u32,
    pub(crate) geometry: EdgeGeometry,
    pub(crate) halfedge: u32,
    pub(crate) twin_halfedge: Option<u32>,
    pub(crate) tolerance: f64,
    pub(crate) chart_seam: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HalfEdgeGeometryUse {
    pub(crate) sense: Orientation,
    pub(crate) pcurve: Option<u32>,
    pub(crate) periodic_lift: [i32; 2],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HalfEdge {
    pub(crate) id: u32,
    pub from: u32,
    pub to: u32,
    pub(crate) twin: Option<u32>,
    pub(crate) next: Option<u32>,
    pub(crate) prev: Option<u32>,
    pub(crate) edge: u32,
    pub(crate) face: Option<u32>,
    pub(crate) loop_ref: Option<u32>,
    pub(crate) wire_ref: Option<u32>,
    pub(crate) geometry_use: HalfEdgeGeometryUse,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Loop {
    pub(crate) id: u32,
    pub(crate) start_halfedge: u32,
    pub(crate) face_ref: u32,
    pub(crate) is_hole: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrimRegion {
    pub(crate) chart: u32,
    pub(crate) uv_bounds: UVBox,
    pub(crate) outer: u32,
    pub(crate) holes: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Face {
    pub id: u32,
    pub(crate) key: String,
    pub surface: u32,
    pub(crate) sense: Orientation,
    pub(crate) trim: TrimRegion,
    pub(crate) shell_ref: Option<u32>,
    pub provenance: FaceProvenance,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Wire {
    pub(crate) id: u32,
    pub(crate) start_halfedge: u32,
    pub(crate) is_closed: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Shell {
    pub(crate) id: u32,
    pub(crate) faces: Vec<u32>,
    pub(crate) is_closed: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SolidRegion {
    pub(crate) outer_shell: u32,
    pub(crate) cavity_shells: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topology {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub halfedges: Vec<HalfEdge>,
    pub(crate) loops: Vec<Loop>,
    pub faces: Vec<Face>,
    pub(crate) wires: Vec<Wire>,
    pub(crate) shells: Vec<Shell>,
}
impl Topology {
    pub(super) fn new() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
            halfedges: Vec::new(),
            loops: Vec::new(),
            faces: Vec::new(),
            wires: Vec::new(),
            shells: Vec::new(),
        }
    }
}
impl Default for Topology {
    fn default() -> Self {
        Self::new()
    }
}
