use openmaths::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub id: u32,
    pub position: Vector3,
    pub outgoing_halfedge: Option<u32>,
    /// Per-vertex modeling tolerance (D2). `None` ⇒ use the global confusion
    /// tolerance. Set higher on vertices known to be imprecise (e.g. produced by
    /// a boolean intersection or de-quantized generated input). Defaulted for
    /// v1 back-compat.
    #[serde(default)]
    pub tolerance: Option<f64>,
}

impl Vertex {
    pub fn new(id: u32, position: Vector3) -> Self {
        Self {
            id,
            position,
            outgoing_halfedge: None,
            tolerance: None,
        }
    }

    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = Some(tolerance);
        self
    }
}
