use std::collections::HashSet;

use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, Edge, Face, Vertex};
use crate::operations::triangulate::triangulate_polygon_with_holes;

const EPSILON: f64 = 1.0e-9;

#[wasm_bindgen]
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum BooleanOperation {
    Union,
    Intersection,
    Difference,
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGBooleanResult {
    brep: Brep,
}

#[wasm_bindgen]
impl OGBooleanResult {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            brep: Brep::new(Uuid::new_v4()),
        }
    }

    /// Runs a voxelized boolean operation from two serialized BReps.
    ///
    /// `voxel_size` acts as the robustness/performance constraint:
    /// - larger values are faster but coarser
    /// - smaller values are slower but more accurate
    #[wasm_bindgen]
    pub fn compute_from_brep_serialized(
        &mut self,
        a_brep_serialized: String,
        b_brep_serialized: String,
        operation: BooleanOperation,
        voxel_size: f64,
    ) -> Result<(), JsValue> {
        let brep_a: Brep = serde_json::from_str(&a_brep_serialized)
            .map_err(|err| JsValue::from_str(&format!("Invalid first BRep payload: {err}")))?;
        let brep_b: Brep = serde_json::from_str(&b_brep_serialized)
            .map_err(|err| JsValue::from_str(&format!("Invalid second BRep payload: {err}")))?;

        if voxel_size <= EPSILON || !voxel_size.is_finite() {
            return Err(JsValue::from_str(
                "voxel_size must be a positive finite number",
            ));
        }

        self.brep = voxel_boolean(&brep_a, &brep_b, operation, voxel_size);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        for face in &self.brep.faces {
            let (face_vertices, holes_vertices) =
                self.brep.get_vertices_and_holes_by_face_id(face.id);
            if face_vertices.len() < 3 {
                continue;
            }

            let triangles = triangulate_polygon_with_holes(&face_vertices, &holes_vertices);
            let all_vertices: Vec<Vector3> = face_vertices
                .into_iter()
                .chain(holes_vertices.into_iter().flatten())
                .collect();

            for triangle in triangles {
                for vertex_index in triangle {
                    let vertex = &all_vertices[vertex_index];
                    vertex_buffer.push(vertex.x);
                    vertex_buffer.push(vertex.y);
                    vertex_buffer.push(vertex.z);
                }
            }
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }
}

impl Default for OGBooleanResult {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct Triangle {
    a: Vector3,
    b: Vector3,
    c: Vector3,
}

fn voxel_boolean(a: &Brep, b: &Brep, operation: BooleanOperation, voxel_size: f64) -> Brep {
    let tris_a = brep_to_triangles(a);
    let tris_b = brep_to_triangles(b);

    if tris_a.is_empty() && tris_b.is_empty() {
        return Brep::new(Uuid::new_v4());
    }

    let bbox = combined_bbox(&tris_a, &tris_b);
    let (min, max) = match bbox {
        Some(value) => value,
        None => return Brep::new(Uuid::new_v4()),
    };

    let nx = (((max.x - min.x) / voxel_size).ceil() as i32).max(1);
    let ny = (((max.y - min.y) / voxel_size).ceil() as i32).max(1);
    let nz = (((max.z - min.z) / voxel_size).ceil() as i32).max(1);

    let mut occupied: HashSet<(i32, i32, i32)> = HashSet::new();

    for ix in 0..nx {
        for iy in 0..ny {
            for iz in 0..nz {
                let center = Vector3::new(
                    min.x + (ix as f64 + 0.5) * voxel_size,
                    min.y + (iy as f64 + 0.5) * voxel_size,
                    min.z + (iz as f64 + 0.5) * voxel_size,
                );

                let in_a = is_inside_mesh(center, &tris_a);
                let in_b = is_inside_mesh(center, &tris_b);

                let keep = match operation {
                    BooleanOperation::Union => in_a || in_b,
                    BooleanOperation::Intersection => in_a && in_b,
                    BooleanOperation::Difference => in_a && !in_b,
                };

                if keep {
                    occupied.insert((ix, iy, iz));
                }
            }
        }
    }

    voxels_to_brep(&occupied, min, voxel_size)
}

fn brep_to_triangles(brep: &Brep) -> Vec<Triangle> {
    let mut triangles = Vec::new();

    for face in &brep.faces {
        let indices = &face.face_indices;
        if indices.len() < 3 {
            continue;
        }

        let base_idx = indices[0] as usize;
        if base_idx >= brep.vertices.len() {
            continue;
        }
        let base = brep.vertices[base_idx].position;

        for i in 1..(indices.len() - 1) {
            let i1 = indices[i] as usize;
            let i2 = indices[i + 1] as usize;
            if i1 >= brep.vertices.len() || i2 >= brep.vertices.len() {
                continue;
            }

            triangles.push(Triangle {
                a: base,
                b: brep.vertices[i1].position,
                c: brep.vertices[i2].position,
            });
        }
    }

    triangles
}

fn combined_bbox(a: &[Triangle], b: &[Triangle]) -> Option<(Vector3, Vector3)> {
    let mut has_any = false;
    let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

    for tri in a.iter().chain(b.iter()) {
        for vertex in [tri.a, tri.b, tri.c] {
            has_any = true;
            min.x = min.x.min(vertex.x);
            min.y = min.y.min(vertex.y);
            min.z = min.z.min(vertex.z);
            max.x = max.x.max(vertex.x);
            max.y = max.y.max(vertex.y);
            max.z = max.z.max(vertex.z);
        }
    }

    if !has_any {
        return None;
    }

    let pad = 1.0e-6;
    Some((
        Vector3::new(min.x - pad, min.y - pad, min.z - pad),
        Vector3::new(max.x + pad, max.y + pad, max.z + pad),
    ))
}

fn is_inside_mesh(point: Vector3, triangles: &[Triangle]) -> bool {
    if triangles.is_empty() {
        return false;
    }

    let directions = [
        Vector3::new(1.0, 0.113, 0.197),
        Vector3::new(0.173, 1.0, 0.271),
        Vector3::new(0.223, 0.317, 1.0),
    ];

    let mut inside_votes = 0;
    for direction in directions {
        let mut hits = 0usize;
        for tri in triangles {
            if ray_intersects_triangle(point, direction, tri) {
                hits += 1;
            }
        }

        if hits % 2 == 1 {
            inside_votes += 1;
        }
    }

    inside_votes >= 2
}

fn ray_intersects_triangle(origin: Vector3, dir: Vector3, tri: &Triangle) -> bool {
    let edge1 = sub(tri.b, tri.a);
    let edge2 = sub(tri.c, tri.a);
    let h = cross(dir, edge2);
    let det = dot(edge1, h);

    if det.abs() < EPSILON {
        return false;
    }

    let inv_det = 1.0 / det;
    let s = sub(origin, tri.a);
    let u = inv_det * dot(s, h);
    if !(-EPSILON..=(1.0 + EPSILON)).contains(&u) {
        return false;
    }

    let q = cross(s, edge1);
    let v = inv_det * dot(dir, q);
    if v < -EPSILON || u + v > 1.0 + EPSILON {
        return false;
    }

    let t = inv_det * dot(edge2, q);
    t > EPSILON
}

fn voxels_to_brep(occupied: &HashSet<(i32, i32, i32)>, min: Vector3, voxel_size: f64) -> Brep {
    let mut brep = Brep::new(Uuid::new_v4());

    if occupied.is_empty() {
        return brep;
    }

    let face_templates = [
        // +X
        (
            (1, 0, 0),
            [
                (1.0, 0.0, 0.0),
                (1.0, 0.0, 1.0),
                (1.0, 1.0, 1.0),
                (1.0, 1.0, 0.0),
            ],
        ),
        // -X
        (
            (-1, 0, 0),
            [
                (0.0, 0.0, 0.0),
                (0.0, 1.0, 0.0),
                (0.0, 1.0, 1.0),
                (0.0, 0.0, 1.0),
            ],
        ),
        // +Y
        (
            (0, 1, 0),
            [
                (0.0, 1.0, 0.0),
                (1.0, 1.0, 0.0),
                (1.0, 1.0, 1.0),
                (0.0, 1.0, 1.0),
            ],
        ),
        // -Y
        (
            (0, -1, 0),
            [
                (0.0, 0.0, 0.0),
                (0.0, 0.0, 1.0),
                (1.0, 0.0, 1.0),
                (1.0, 0.0, 0.0),
            ],
        ),
        // +Z
        (
            (0, 0, 1),
            [
                (0.0, 0.0, 1.0),
                (0.0, 1.0, 1.0),
                (1.0, 1.0, 1.0),
                (1.0, 0.0, 1.0),
            ],
        ),
        // -Z
        (
            (0, 0, -1),
            [
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ],
        ),
    ];

    let mut vertex_id: u32 = 0;
    let mut edge_id: u32 = 0;
    let mut face_id: u32 = 0;

    for &(ix, iy, iz) in occupied {
        for (offset, corners) in face_templates {
            let neighbor = (ix + offset.0, iy + offset.1, iz + offset.2);
            if occupied.contains(&neighbor) {
                continue;
            }

            let mut face_indices = Vec::with_capacity(4);

            for corner in corners {
                let position = Vector3::new(
                    min.x + (ix as f64 + corner.0) * voxel_size,
                    min.y + (iy as f64 + corner.1) * voxel_size,
                    min.z + (iz as f64 + corner.2) * voxel_size,
                );

                brep.vertices.push(Vertex::new(vertex_id, position));
                face_indices.push(vertex_id);
                vertex_id += 1;
            }

            let e0 = Edge::new(edge_id, face_indices[0], face_indices[1]);
            edge_id += 1;
            let e1 = Edge::new(edge_id, face_indices[1], face_indices[2]);
            edge_id += 1;
            let e2 = Edge::new(edge_id, face_indices[2], face_indices[3]);
            edge_id += 1;
            let e3 = Edge::new(edge_id, face_indices[3], face_indices[0]);
            edge_id += 1;

            brep.edges.extend([e0, e1, e2, e3]);
            brep.faces.push(Face::new(face_id, face_indices));
            face_id += 1;
        }
    }

    brep
}

fn sub(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn dot(a: Vector3, b: Vector3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cross(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

#[cfg(test)]
mod tests {
    use super::{BooleanOperation, OGBooleanResult};
    use crate::primitives::cuboid::OGCuboid;
    use openmaths::Vector3;

    fn unit_cuboid(id: &str, center: Vector3) -> OGCuboid {
        let mut cuboid = OGCuboid::new(id.to_string());
        cuboid.set_config(center, 1.0, 1.0, 1.0);
        cuboid
    }

    #[test]
    fn boolean_union_produces_non_empty_geometry() {
        let a = unit_cuboid("a", Vector3::new(0.0, 0.0, 0.0));
        let b = unit_cuboid("b", Vector3::new(0.5, 0.0, 0.0));

        let mut result = OGBooleanResult::new();
        result
            .compute_from_brep_serialized(
                a.get_brep_serialized(),
                b.get_brep_serialized(),
                BooleanOperation::Union,
                0.25,
            )
            .expect("boolean operation should succeed");

        assert!(result.get_geometry_serialized().len() > 2);
    }

    #[test]
    fn boolean_difference_produces_non_empty_geometry() {
        let a = unit_cuboid("a", Vector3::new(0.0, 0.0, 0.0));
        let b = unit_cuboid("b", Vector3::new(0.4, 0.0, 0.0));

        let mut result = OGBooleanResult::new();
        result
            .compute_from_brep_serialized(
                a.get_brep_serialized(),
                b.get_brep_serialized(),
                BooleanOperation::Difference,
                0.2,
            )
            .expect("boolean operation should succeed");

        assert!(result.get_geometry_serialized().len() > 2);
    }
}
