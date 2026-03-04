/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Line Primitive for OpenGeometry.
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
pub struct OGLine {
    id: String,
    brep: Brep,
    start: Vector3,
    end: Vector3,
}

impl Drop for OGLine {
    fn drop(&mut self) {
        self.brep.clear();
        self.id.clear();
    }
}

#[wasm_bindgen]
impl OGLine {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGLine {
        OGLine {
            id,
            start: Vector3::new(1.0, 0.0, 0.0),
            end: Vector3::new(-1.0, 0.0, 0.0),
            brep: Brep::new(Uuid::new_v4()),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(&mut self, start: Vector3, end: Vector3) -> Result<(), JsValue> {
        self.start = start;
        self.end = end;
        self.generate_geometry()
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&[self.start, self.end]);
        builder
            .add_wire(&[0, 1], false)
            .map_err(|err| JsValue::from_str(&format!("Failed to build line wire: {}", err)))?;

        self.brep = builder
            .build()
            .map_err(|err| JsValue::from_str(&format!("Failed to finalize line BREP: {}", err)))?;

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

        for vertex_id in self.brep.get_wire_vertex_indices(0) {
            if let Some(vertex) = self.brep.vertices.get(vertex_id as usize) {
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

    pub fn get_dxf_serialized(&self) -> String {
        String::new()
    }
}

impl OGLine {
    pub fn brep(&self) -> &Brep {
        &self.brep
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
        let points = vec![self.start, self.end];
        offset_path(&points, distance, Some(false), options)
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
