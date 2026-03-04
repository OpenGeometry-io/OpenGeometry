use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
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
    ) -> Result<(), JsValue> {
        self.center = center;
        self.radius = radius.max(1.0e-6);
        self.width_segments = width_segments.max(3);
        self.height_segments = height_segments.max(2);

        self.generate_brep()
    }

    pub fn generate_brep(&mut self) -> Result<(), JsValue> {
        self.clean_geometry();
        self.generate_geometry()
    }

    pub fn clean_geometry(&mut self) {
        self.brep.clear();
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        let width = self.width_segments.max(3) as usize;
        let height = self.height_segments.max(2) as usize;

        let mut vertices = Vec::new();
        // Top pole.
        vertices.push(Vector3::new(
            self.center.x,
            self.center.y + self.radius,
            self.center.z,
        ));

        // Intermediate rings.
        for iy in 1..height {
            let v = iy as f64 / height as f64;
            let theta = v * std::f64::consts::PI;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for ix in 0..width {
                let u = ix as f64 / width as f64;
                let phi = u * std::f64::consts::PI * 2.0;
                let sin_phi = phi.sin();
                let cos_phi = phi.cos();

                vertices.push(Vector3::new(
                    self.center.x + self.radius * sin_theta * cos_phi,
                    self.center.y + self.radius * cos_theta,
                    self.center.z + self.radius * sin_theta * sin_phi,
                ));
            }
        }

        let bottom_id = vertices.len() as u32;
        vertices.push(Vector3::new(
            self.center.x,
            self.center.y - self.radius,
            self.center.z,
        ));

        let ring_vertex = |ring: usize, ix: usize| -> u32 {
            // ring is 1..height-1
            1 + ((ring - 1) * width + ix) as u32
        };

        let mut faces: Vec<Vec<u32>> = Vec::new();

        // Top cap.
        for ix in 0..width {
            let next = (ix + 1) % width;
            faces.push(vec![0, ring_vertex(1, next), ring_vertex(1, ix)]);
        }

        // Body quads split into triangles.
        if height > 2 {
            for ring in 1..(height - 1) {
                for ix in 0..width {
                    let next = (ix + 1) % width;
                    let a = ring_vertex(ring, ix);
                    let b = ring_vertex(ring, next);
                    let c = ring_vertex(ring + 1, next);
                    let d = ring_vertex(ring + 1, ix);

                    faces.push(vec![a, b, c]);
                    faces.push(vec![a, c, d]);
                }
            }
        }

        // Bottom cap.
        let last_ring = height - 1;
        for ix in 0..width {
            let next = (ix + 1) % width;
            faces.push(vec![
                bottom_id,
                ring_vertex(last_ring, ix),
                ring_vertex(last_ring, next),
            ]);
        }

        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&vertices);

        for face in &faces {
            builder.add_face(face, &[]).map_err(|err| {
                JsValue::from_str(&format!("Failed to build sphere face: {}", err))
            })?;
        }

        builder
            .add_shell_from_all_faces(true)
            .map_err(|err| JsValue::from_str(&format!("Failed to build sphere shell: {}", err)))?;

        self.brep = builder.build().map_err(|err| {
            JsValue::from_str(&format!("Failed to finalize sphere BREP: {}", err))
        })?;

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
            let face_vertices = self.brep.get_vertices_by_face_id(face.id);
            if face_vertices.len() != 3 {
                continue;
            }

            for vertex in face_vertices {
                vertex_buffer.push(vertex.x);
                vertex_buffer.push(vertex.y);
                vertex_buffer.push(vertex.z);
            }
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_outline_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        for (start_id, end_id) in self.brep.collect_outline_segments() {
            let Some(start_vertex) = self.brep.vertices.get(start_id as usize) else {
                continue;
            };
            let Some(end_vertex) = self.brep.vertices.get(end_id as usize) else {
                continue;
            };

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
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
