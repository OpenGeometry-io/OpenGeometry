/*
 * Geometry Module
 *
 * The final geometry is stored in geometry buffer.
 * It has trangles vertices and outline vertices, nothing more.
 * It is the final stage of geometry before it is sent to GPU for rendering.
 * The geometry buffer is created from BREP and then sent to GPU or other rendering systems.
 */

use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::spatial::placement::Placement3D;

#[derive(Clone, Serialize, Deserialize)]
pub struct GeometryBuffer {
    id: Uuid,
    // flat vertices for triangles, 3 values per vertex (x, y, z)
    pub vertices: Vec<f64>,
    pub outline_vertices: Vec<f64>,
    // TODO: add other geometry buffer properties such as normals, uvs, etc.
}

impl GeometryBuffer {
    // Why Getter and Setter - https://github.com/rustwasm/wasm-bindgen/issues/1775
    pub fn set_id(&mut self, id: Uuid) {
        self.id = id;
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn new(id: Uuid) -> GeometryBuffer {
        GeometryBuffer {
            id,
            vertices: Vec::new(),
            outline_vertices: Vec::new(),
        }
    }

    /**
     * Applies the given placement's world transformation to the geometry buffer. This function transforms all vertices in the geometry buffer from their local space to world space using the transformation defined by the placement. The transformation includes translation, rotation, and scaling as specified in the placement's world matrix. After applying this function, the vertices in the geometry buffer will be updated to reflect their new positions in world space, which can then be used for rendering or further processing.
     */
    pub fn apply_transform(&mut self, placement: &Placement3D) {
        let placement_matrix = placement.world_matrix();
        // TODO: Apply the placement_matrix to all vertices in self.vertices and self.outline_vertices, figure out if outline vertices should be transformed differently or not.
    }

    pub fn reset_geometry(&mut self) {
        self.vertices.clear();
        self.outline_vertices.clear();
    }

    pub fn get_center(&self) -> Vector3 {
        let mut center = Vector3::new(0.0, 0.0, 0.0);
        // TODO: calculate center from vertices

        center
    }
}
