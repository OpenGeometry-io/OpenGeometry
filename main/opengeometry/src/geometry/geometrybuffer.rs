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
        transform_flat_vertex_buffer(&mut self.vertices, &placement_matrix);
        transform_flat_vertex_buffer(&mut self.outline_vertices, &placement_matrix);
    }

    pub fn transformed(&self, placement: &Placement3D) -> GeometryBuffer {
        let mut geometry = self.clone();
        geometry.apply_transform(placement);
        geometry
    }

    pub fn reset_geometry(&mut self) {
        self.vertices.clear();
        self.outline_vertices.clear();
    }

    pub fn get_center(&self) -> Vector3 {
        let Some(first_vertex) = self.vertices.chunks_exact(3).next() else {
            return Vector3::new(0.0, 0.0, 0.0);
        };

        let mut min_x = first_vertex[0];
        let mut min_y = first_vertex[1];
        let mut min_z = first_vertex[2];
        let mut max_x = first_vertex[0];
        let mut max_y = first_vertex[1];
        let mut max_z = first_vertex[2];

        for vertex in self.vertices.chunks_exact(3).skip(1) {
            min_x = min_x.min(vertex[0]);
            min_y = min_y.min(vertex[1]);
            min_z = min_z.min(vertex[2]);
            max_x = max_x.max(vertex[0]);
            max_y = max_y.max(vertex[1]);
            max_z = max_z.max(vertex[2]);
        }

        Vector3::new(
            (min_x + max_x) * 0.5,
            (min_y + max_y) * 0.5,
            (min_z + max_z) * 0.5,
        )
    }
}

fn transform_flat_vertex_buffer(buffer: &mut [f64], matrix: &openmaths::Matrix4) {
    for vertex in buffer.chunks_exact_mut(3) {
        let mut point = Vector3::new(vertex[0], vertex[1], vertex[2]);
        point.apply_matrix4(matrix.clone());
        vertex[0] = point.x;
        vertex[1] = point.y;
        vertex[2] = point.z;
    }
}
