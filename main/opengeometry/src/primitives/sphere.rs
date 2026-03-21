use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::spatial::placement::Placement3D;
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
    placement: Placement3D,
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
            placement: Placement3D::new(),
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
        self.placement.set_anchor(self.center);

        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn set_center(&mut self, center: Vector3) {
        self.center = center;
        self.placement.set_anchor(self.center);
    }

    #[wasm_bindgen]
    pub fn set_transform(
        &mut self,
        position: Vector3,
        rotation: Vector3,
        scale: Vector3,
    ) -> Result<(), JsValue> {
        self.placement
            .set_transform(position, rotation, scale)
            .map_err(|err| JsValue::from_str(&err))
    }

    #[wasm_bindgen]
    pub fn set_translation(&mut self, translation: Vector3) {
        self.placement.set_translation(translation);
    }

    #[wasm_bindgen]
    pub fn set_rotation(&mut self, rotation: Vector3) {
        self.placement.set_rotation(rotation);
    }

    #[wasm_bindgen]
    pub fn set_scale(&mut self, scale: Vector3) -> Result<(), JsValue> {
        self.placement
            .set_scale(scale)
            .map_err(|err| JsValue::from_str(&err))
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
        vertices.push(Vector3::new(0.0, self.radius, 0.0));

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
                    self.radius * sin_theta * cos_phi,
                    self.radius * cos_theta,
                    self.radius * sin_theta * sin_phi,
                ));
            }
        }

        let bottom_id = vertices.len() as u32;
        vertices.push(Vector3::new(0.0, -self.radius, 0.0));

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
        serde_json::to_string(&self.world_brep()).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_local_brep_serialized(&self) -> String {
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let world_brep = self.world_brep();
        serde_json::to_string(&world_brep.get_triangle_vertex_buffer()).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_local_geometry_serialized(&self) -> String {
        serde_json::to_string(&self.brep.get_triangle_vertex_buffer()).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_buffer(&self) -> Vec<f64> {
        self.world_brep().get_triangle_vertex_buffer()
    }

    #[wasm_bindgen]
    pub fn get_local_geometry_buffer(&self) -> Vec<f64> {
        self.brep.get_triangle_vertex_buffer()
    }

    #[wasm_bindgen]
    pub fn get_outline_geometry_serialized(&self) -> String {
        let world_brep = self.world_brep();
        serde_json::to_string(&world_brep.get_outline_vertex_buffer()).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_local_outline_geometry_serialized(&self) -> String {
        serde_json::to_string(&self.brep.get_outline_vertex_buffer()).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_outline_geometry_buffer(&self) -> Vec<f64> {
        self.world_brep().get_outline_vertex_buffer()
    }

    #[wasm_bindgen]
    pub fn get_local_outline_geometry_buffer(&self) -> Vec<f64> {
        self.brep.get_outline_vertex_buffer()
    }

    #[wasm_bindgen]
    pub fn get_anchor(&self) -> Vector3 {
        self.placement.anchor
    }
}

impl OGSphere {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn world_brep(&self) -> Brep {
        self.brep.transformed(&self.placement)
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        let world_brep = self.world_brep();
        project_brep_to_scene(&world_brep, camera, hlr)
    }
}
