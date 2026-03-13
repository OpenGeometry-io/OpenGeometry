use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
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
        Ok(())
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        let segment_count = self.segments.max(1);
        let angle_step = (self.end_angle - self.start_angle) / segment_count as f64;

        let mut points = Vec::with_capacity((segment_count + 1) as usize);
        let mut angle = self.start_angle;
        for _ in 0..=segment_count {
            let x = self.center.x + self.radius * angle.cos();
            let y = self.center.y;
            let z = self.center.z + self.radius * angle.sin();
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
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        if let Some(wire) = self.brep.wires.first() {
            let mut wire_vertices = self.brep.get_wire_vertex_indices(wire.id);
            if wire.is_closed {
                if let Some(first) = wire_vertices.first().copied() {
                    wire_vertices.push(first);
                }
            }

            for vertex_id in wire_vertices {
                if let Some(vertex) = self.brep.vertices.get(vertex_id as usize) {
                    vertex_buffer.push(vertex.position.x);
                    vertex_buffer.push(vertex.position.y);
                    vertex_buffer.push(vertex.position.z);
                }
            }
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }
}

impl OGArc {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
