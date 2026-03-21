use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::spatial::placement::Placement3D;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGArc {
    id: String,
    center: Vector3,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    segments: u32,
    placement: Placement3D,
    brep: Brep,
}

#[wasm_bindgen]
impl OGArc {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGArc {
        let internal_id = Uuid::new_v4();

        OGArc {
            id,
            center: Vector3::new(0.0, 0.0, 0.0),
            radius: 1.0,
            start_angle: 0.0,
            end_angle: 2.0 * std::f64::consts::PI,
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
        start_angle: f64,
        end_angle: f64,
        segments: u32,
    ) -> Result<(), JsValue> {
        self.center = center;
        self.radius = radius;
        self.start_angle = start_angle;
        self.end_angle = end_angle;
        self.segments = segments.max(1);
        self.placement.set_anchor(self.center);
        self.generate_geometry()
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

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        let segment_count = self.segments.max(1);
        let angle_step = (self.end_angle - self.start_angle) / segment_count as f64;

        let mut points = Vec::with_capacity((segment_count + 1) as usize);
        let mut angle = self.start_angle;
        for _ in 0..=segment_count {
            let x = self.radius * angle.cos();
            let y = 0.0;
            let z = self.radius * angle.sin();
            points.push(Vector3::new(x, y, z));
            angle += angle_step;
        }

        let is_closed =
            (self.end_angle - self.start_angle).abs() >= 2.0 * std::f64::consts::PI - 1.0e-9;

        if is_closed && points.len() > 2 {
            let first = points[0];
            let last = *points.last().unwrap();
            let dx = first.x - last.x;
            let dy = first.y - last.y;
            let dz = first.z - last.z;
            if dx * dx + dy * dy + dz * dz <= 1.0e-12 {
                points.pop();
            }
        }

        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&points);

        if points.len() >= 2 {
            let indices: Vec<u32> = (0..points.len() as u32).collect();
            builder
                .add_wire(&indices, is_closed && points.len() > 2)
                .map_err(|err| JsValue::from_str(&format!("Failed to build arc wire: {}", err)))?;
        }

        self.brep = builder
            .build()
            .map_err(|err| JsValue::from_str(&format!("Failed to finalize arc BREP: {}", err)))?;

        Ok(())
    }

    #[wasm_bindgen]
    pub fn dispose_points(&mut self) {
        self.brep.clear();
    }

    #[wasm_bindgen]
    pub fn destroy(&mut self) {
        self.brep.clear();
        self.id.clear();
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
        serde_json::to_string(&wire_geometry_buffer(&self.world_brep())).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_local_geometry_serialized(&self) -> String {
        serde_json::to_string(&wire_geometry_buffer(&self.brep)).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_buffer(&self) -> Vec<f64> {
        wire_geometry_buffer(&self.world_brep())
    }

    #[wasm_bindgen]
    pub fn get_local_geometry_buffer(&self) -> Vec<f64> {
        wire_geometry_buffer(&self.brep)
    }

    #[wasm_bindgen]
    pub fn get_anchor(&self) -> Vector3 {
        self.placement.anchor
    }
}

impl OGArc {
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

fn wire_geometry_buffer(brep: &Brep) -> Vec<f64> {
    let Some(wire) = brep.wires.first() else {
        return Vec::new();
    };

    brep.get_wire_vertex_buffer(wire.id, wire.is_closed)
}
