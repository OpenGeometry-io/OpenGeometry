use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, Edge, Face, Vertex};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGSphere {
    id: String,
    center: Vector3,
    radius: f64,
    width_segments: u32,
    height_segments: u32,
    brep: Brep,
}

#[wasm_bindgen]
impl OGSphere {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGSphere {
        let internal_id = Uuid::new_v4();

        OGSphere {
            id,
            center: Vector3::new(0.0, 0.0, 0.0),
            radius: 1.0,
            width_segments: 24,
            height_segments: 16,
            brep: Brep::new(internal_id),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(
        &mut self,
        center: Vector3,
        radius: f64,
        width_segments: u32,
        height_segments: u32,
    ) {
        self.center = center;
        self.radius = radius.max(1.0e-6);
        self.width_segments = width_segments.max(3);
        self.height_segments = height_segments.max(2);

        self.generate_brep();
    }

    pub fn generate_brep(&mut self) {
        self.clean_geometry();
        self.generate_geometry();
    }

    pub fn clean_geometry(&mut self) {
        self.brep.clear();
        self.brep.holes.clear();
        self.brep.hole_edges.clear();
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) {
        self.clean_geometry();

        let width = self.width_segments.max(3) as usize;
        let height = self.height_segments.max(2) as usize;

        let mut vertex_indices = vec![vec![0u32; width + 1]; height + 1];

        for iy in 0..=height {
            let v = iy as f64 / height as f64;
            let theta = v * std::f64::consts::PI;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for ix in 0..=width {
                let u = ix as f64 / width as f64;
                let phi = u * std::f64::consts::PI * 2.0;
                let sin_phi = phi.sin();
                let cos_phi = phi.cos();

                let x = self.center.x + self.radius * sin_theta * cos_phi;
                let y = self.center.y + self.radius * cos_theta;
                let z = self.center.z + self.radius * sin_theta * sin_phi;

                let id = self.brep.get_vertex_count();
                self.brep
                    .vertices
                    .push(Vertex::new(id, Vector3::new(x, y, z)));
                vertex_indices[iy][ix] = id;
            }
        }

        let mut edge_set: HashSet<(u32, u32)> = HashSet::new();
        let mut next_face_id: u32 = 0;

        for iy in 0..height {
            for ix in 0..width {
                let a = vertex_indices[iy][ix];
                let b = vertex_indices[iy][ix + 1];
                let c = vertex_indices[iy + 1][ix];
                let d = vertex_indices[iy + 1][ix + 1];

                if iy != 0 {
                    self.brep.faces.push(Face::new(next_face_id, vec![a, c, b]));
                    next_face_id += 1;

                    Self::push_edge_if_new(&mut self.brep, &mut edge_set, a, c);
                    Self::push_edge_if_new(&mut self.brep, &mut edge_set, c, b);
                    Self::push_edge_if_new(&mut self.brep, &mut edge_set, b, a);
                }

                if iy != (height - 1) {
                    self.brep.faces.push(Face::new(next_face_id, vec![b, c, d]));
                    next_face_id += 1;

                    Self::push_edge_if_new(&mut self.brep, &mut edge_set, b, c);
                    Self::push_edge_if_new(&mut self.brep, &mut edge_set, c, d);
                    Self::push_edge_if_new(&mut self.brep, &mut edge_set, d, b);
                }
            }
        }
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        for face in &self.brep.faces {
            if face.face_indices.len() != 3 {
                continue;
            }

            for vertex_index in &face.face_indices {
                let vertex = &self.brep.vertices[*vertex_index as usize];
                vertex_buffer.push(vertex.position.x);
                vertex_buffer.push(vertex.position.y);
                vertex_buffer.push(vertex.position.z);
            }
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_outline_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        for edge in &self.brep.edges {
            let start_vertex = self.brep.vertices[edge.v1 as usize].clone();
            let end_vertex = self.brep.vertices[edge.v2 as usize].clone();

            vertex_buffer.push(start_vertex.position.x);
            vertex_buffer.push(start_vertex.position.y);
            vertex_buffer.push(start_vertex.position.z);
            vertex_buffer.push(end_vertex.position.x);
            vertex_buffer.push(end_vertex.position.y);
            vertex_buffer.push(end_vertex.position.z);
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }
}

impl OGSphere {
    fn push_edge_if_new(brep: &mut Brep, edge_set: &mut HashSet<(u32, u32)>, v1: u32, v2: u32) {
        let key = if v1 < v2 { (v1, v2) } else { (v2, v1) };
        if edge_set.insert(key) {
            brep.edges.push(Edge::new(brep.get_edge_count(), v1, v2));
        }
    }

    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
