/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Swept Shape Primitive for OpenGeometry.
 * 
 * A high-level primitive for creating swept shapes by sweeping a profile
 * along a path. This provides an easy-to-use interface for sweep operations.
 */

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use crate::brep::Brep;
use crate::geometry::path::Path;
use crate::operations::sweep::sweep_profile_along_path;
use crate::utility::bgeometry::BufferGeometry;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGSweptShape {
    id: String,
    brep: Brep,
    geometry: BufferGeometry,
}

#[wasm_bindgen]
impl OGSweptShape {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGSweptShape {
        let internal_id = Uuid::new_v4();

        OGSweptShape {
            id,
            brep: Brep::new(internal_id),
            geometry: BufferGeometry::new(internal_id),
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    /// Create a swept shape from a profile B-Rep and a path
    /// This is a convenience method that doesn't require implementing Path trait
    #[wasm_bindgen]
    pub fn sweep_along_points(&mut self, profile_vertices: Vec<Vector3>, path_points: Vec<Vector3>) {
        // Create a simple profile B-Rep from the vertices
        let mut profile_brep = Brep::new(Uuid::new_v4());
        
        // Add vertices to the profile
        for vertex in &profile_vertices {
            profile_brep.add_vertex(*vertex);
        }

        // Add a single face connecting all vertices (assumes convex profile)
        if profile_vertices.len() >= 3 {
            let indices: Vec<u32> = (0..profile_vertices.len() as u32).collect();
            profile_brep.add_face(indices);
        }

        // Create a simple path implementation
        struct SimplePolylinePath {
            points: Vec<Vector3>,
        }

        impl Path for SimplePolylinePath {
            fn get_points(&self) -> Vec<Vector3> {
                self.points.clone()
            }

            fn get_frames(&self) -> Vec<openmaths::Matrix4> {
                let mut frames = Vec::new();
                if self.points.len() < 2 {
                    return frames;
                }

                for i in 0..self.points.len() {
                    let position = self.points[i];
                    let tangent = if i < self.points.len() - 1 {
                        let next_pos = self.points[i + 1];
                        next_pos.clone().subtract(&position).normalize()
                    } else {
                        let prev_pos = self.points[i - 1];
                        position.clone().subtract(&prev_pos).normalize()
                    };

                    // Simple approach for up vector
                    let mut up = Vector3::new(0.0, 1.0, 0.0);
                    if tangent.y.abs() > 0.999 {
                        up = Vector3::new(0.0, 0.0, 1.0);
                    }

                    let right = tangent.cross(&up).normalize();
                    let new_up = right.cross(&tangent).normalize();

                    // Create transformation matrix
                    let matrix = openmaths::Matrix4::set(
                        right.x, new_up.x, tangent.x, position.x,
                        right.y, new_up.y, tangent.y, position.y,
                        right.z, new_up.z, tangent.z, position.z,
                        0.0, 0.0, 0.0, 1.0,
                    );

                    frames.push(matrix);
                }
                frames
            }
        }

        let path = SimplePolylinePath { points: path_points };
        
        // Perform the sweep operation
        self.brep = sweep_profile_along_path(&profile_brep, &path);
    }

    /// Generate geometry from the B-Rep for rendering
    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) {
        self.geometry.clear();

        // Convert B-Rep faces to renderable geometry
        for face in &self.brep.faces {
            let face_vertices: Vec<Vector3> = face.face_indices.iter()
                .map(|&index| self.brep.vertices[index as usize].position)
                .collect();

            // For quads (4 vertices), split into two triangles
            if face_vertices.len() == 4 {
                // First triangle: 0, 1, 2
                self.geometry.add_vertex(face_vertices[0]);
                self.geometry.add_vertex(face_vertices[1]);
                self.geometry.add_vertex(face_vertices[2]);

                // Second triangle: 0, 2, 3
                self.geometry.add_vertex(face_vertices[0]);
                self.geometry.add_vertex(face_vertices[2]);
                self.geometry.add_vertex(face_vertices[3]);
            } else if face_vertices.len() == 3 {
                // Triangle: add directly
                self.geometry.add_vertex(face_vertices[0]);
                self.geometry.add_vertex(face_vertices[1]);
                self.geometry.add_vertex(face_vertices[2]);
            }
            // For faces with more than 4 vertices, triangulation would be needed
        }
    }

    /// Get the generated geometry as a serialized string for TypeScript
    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();
        
        for vertex in self.geometry.get_vertices() {
            vertex_buffer.push(vertex.x);
            vertex_buffer.push(vertex.y);
            vertex_buffer.push(vertex.z);
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }

    /// Get the B-Rep data as a serialized string
    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        serde_json::to_string(&self.brep).unwrap()
    }

    /// Clear the swept shape data
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.brep.clear();
        self.geometry.clear();
    }
}