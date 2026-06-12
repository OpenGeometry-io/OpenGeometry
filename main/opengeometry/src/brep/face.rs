use openmaths::Vector3;
use serde::{Deserialize, Serialize};

use super::geometry::SurfaceGeometry;

#[derive(Clone, Serialize, Deserialize)]
pub struct Face {
    pub id: u32,
    pub normal: Vector3,
    pub outer_loop: u32,
    pub inner_loops: Vec<u32>,
    pub shell_ref: Option<u32>,
    /// Exact analytic surface of this face (D1). `None` ⇒ a general planar
    /// polygon whose plane is implied by its loop. Defaulted for v1 back-compat.
    #[serde(default)]
    pub surface: Option<SurfaceGeometry>,
}

impl Face {
    pub fn new(
        id: u32,
        normal: Vector3,
        outer_loop: u32,
        inner_loops: Vec<u32>,
        shell_ref: Option<u32>,
    ) -> Self {
        Self {
            id,
            normal,
            outer_loop,
            inner_loops,
            shell_ref,
            surface: None,
        }
    }

    pub fn set_normal(&mut self, normal: Vector3) {
        self.normal = normal;
    }

    pub fn set_surface(&mut self, surface: SurfaceGeometry) {
        self.surface = Some(surface);
    }
}
