use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::extrude::extrude_brep_face;
use crate::spatial::placement::Placement3D;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGCylinder {
    id: String,
    center: Vector3,
    radius: f64,
    height: f64,
    angle: f64,
    segments: u32,
    placement: Placement3D,
    brep: Brep,
}

#[wasm_bindgen]
impl OGCylinder {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGCylinder {
        let internal_id = Uuid::new_v4();

        OGCylinder {
            id,
            center: Vector3::new(0.0, 0.0, 0.0),
            radius: 1.0,
            height: 1.0,
            angle: 2.0 * std::f64::consts::PI,
            segments: 32,
            placement: Placement3D::new(),
            brep: Brep::new(internal_id),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(
        &mut self,
        center: Vector3,
        radius: f64,
        height: f64,
        angle: f64,
        segments: u32,
    ) -> Result<(), JsValue> {
        self.center = center;
        self.radius = radius.max(1.0e-6);
        self.height = height;
        self.angle = angle;
        self.segments = segments.max(3);
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
        let half_height = self.height / 2.0;
        let mut base_points = Vec::new();
        let full_circle = self.angle >= 2.0 * std::f64::consts::PI - 1.0e-9;

        if !full_circle {
            base_points.push(Vector3::new(0.0, -half_height, 0.0));
        }

        let segment_count = self.segments.max(3);
        let steps = if full_circle {
            segment_count
        } else {
            segment_count + 1
        };
        let angle_step = if full_circle {
            self.angle / segment_count as f64
        } else {
            self.angle / (steps - 1) as f64
        };

        let mut angle: f64 = 0.0;
        for _ in 0..steps {
            let x = self.radius * angle.cos();
            let y = -half_height;
            let z = self.radius * angle.sin();
            base_points.push(Vector3::new(x, y, z));
            angle += angle_step;
        }

        if full_circle && base_points.len() > 3 {
            let first = base_points[0];
            let last = *base_points.last().unwrap();
            let dx = first.x - last.x;
            let dy = first.y - last.y;
            let dz = first.z - last.z;
            if dx * dx + dy * dy + dz * dz <= 1.0e-12 {
                base_points.pop();
            }
        }

        let mut base_builder = BrepBuilder::new(Uuid::new_v4());
        base_builder.add_vertices(&base_points);
        let base_loop_indices: Vec<u32> = (0..base_points.len() as u32).collect();
        base_builder
            .add_face(&base_loop_indices, &[])
            .map_err(|err| {
                JsValue::from_str(&format!("Failed to build cylinder base face: {}", err))
            })?;

        let base_brep = base_builder.build().map_err(|err| {
            JsValue::from_str(&format!("Failed to finalize cylinder base: {}", err))
        })?;

        self.brep = extrude_brep_face(base_brep, self.height);

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

impl OGCylinder {
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
