use serde::{Deserialize, Serialize};

use super::geometry::CurveGeometry;

#[derive(Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: u32,
    pub halfedge: u32,
    pub twin_halfedge: Option<u32>,
    /// Exact analytic geometry of this edge (D1). `None` ⇒ a straight segment
    /// between its endpoint vertices. Defaulted so legacy (v1) B-rep JSON that
    /// predates analytic geometry still deserializes.
    #[serde(default)]
    pub curve: Option<CurveGeometry>,
    /// Per-edge modeling tolerance (D2). `None` ⇒ use the global confusion
    /// tolerance. Intersection edges from a boolean carry their own, looser
    /// tolerance. Defaulted for v1 back-compat.
    #[serde(default)]
    pub tolerance: Option<f64>,
}

impl Edge {
    pub fn new(id: u32, halfedge: u32, twin_halfedge: Option<u32>) -> Self {
        Self {
            id,
            halfedge,
            twin_halfedge,
            curve: None,
            tolerance: None,
        }
    }

    pub fn with_curve(mut self, curve: CurveGeometry) -> Self {
        self.curve = Some(curve);
        self
    }

    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = Some(tolerance);
        self
    }
}
