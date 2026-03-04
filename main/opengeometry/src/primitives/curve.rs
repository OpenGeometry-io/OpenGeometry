/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Curve Primitive for OpenGeometry.
 */
use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::offset::{offset_path, OffsetOptions, OffsetResult};
use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGCurve {
    id: String,
    control_points: Vec<Vector3>,
    brep: Brep,
}

#[wasm_bindgen]
impl OGCurve {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGCurve {
        OGCurve {
            id,
            control_points: Vec::new(),
            brep: Brep::new(Uuid::new_v4()),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(&mut self, control_points: Vec<Vector3>) -> Result<(), JsValue> {
        self.control_points = control_points;
        self.generate_geometry()
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        if self.control_points.is_empty() {
            self.brep.clear();
            return Ok(());
        }

        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&self.control_points);

        if self.control_points.len() >= 2 {
            let indices: Vec<u32> = (0..self.control_points.len() as u32).collect();
            builder.add_wire(&indices, false).map_err(|err| {
                JsValue::from_str(&format!("Failed to build curve wire: {}", err))
            })?;
        }

        self.brep = builder
            .build()
            .map_err(|err| JsValue::from_str(&format!("Failed to finalize curve BREP: {}", err)))?;

        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        if let Some(wire) = self.brep.wires.first() {
            for vertex_id in self.brep.get_wire_vertex_indices(wire.id) {
                if let Some(vertex) = self.brep.vertices.get(vertex_id as usize) {
                    vertex_buffer.push(vertex.position.x);
                    vertex_buffer.push(vertex.position.y);
                    vertex_buffer.push(vertex.position.z);
                }
            }
        } else {
            for vertex in &self.brep.vertices {
                vertex_buffer.push(vertex.position.x);
                vertex_buffer.push(vertex.position.y);
                vertex_buffer.push(vertex.position.z);
            }
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_offset_serialized(
        &self,
        distance: f64,
        acute_threshold_degrees: f64,
        bevel: bool,
    ) -> String {
        let result = self.get_offset_result(distance, acute_threshold_degrees, bevel);
        serde_json::to_string(&result).unwrap()
    }

    #[wasm_bindgen]
    pub fn dispose(&mut self) {
        self.brep.clear();
        self.control_points.clear();
    }
}

impl OGCurve {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn get_raw_points(&self) -> Vec<Vector3> {
        self.control_points.clone()
    }

    pub fn get_offset_result(
        &self,
        distance: f64,
        acute_threshold_degrees: f64,
        bevel: bool,
    ) -> OffsetResult {
        let options = OffsetOptions {
            bevel,
            acute_threshold_degrees,
        };
        offset_path(&self.control_points, distance, Some(false), options)
    }

    pub fn get_offset_points(
        &self,
        distance: f64,
        acute_threshold_degrees: f64,
        bevel: bool,
    ) -> Vec<Vector3> {
        self.get_offset_result(distance, acute_threshold_degrees, bevel)
            .points
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
