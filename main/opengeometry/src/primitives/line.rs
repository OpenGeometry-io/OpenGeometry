/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Line Primitive for OpenGeometry.
 *
 * A Line is defined by two points.
 * This line would only have two points, else it becomes a polyline.
 * Created with two arbitrary points, start and end.
 */
use crate::brep::{Brep, Edge, Vertex};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::offset::{offset_path, OffsetOptions, OffsetResult};
use dxf::entities::*;
use dxf::Drawing;
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
        // TODO: Add dispose for Vector3 in OpenMaths
        // self.start.dispose();
        // self.end.dispose();
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
    pub fn set_config(&mut self, start: Vector3, end: Vector3) {
        self.brep.clear();
        self.start = start;
        self.end = end;
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) {
        self.brep.clear();

        let start_vertex = Vertex::new(0, self.start);
        let end_vertex = Vertex::new(1, self.end);

        self.brep.vertices.push(start_vertex);
        self.brep.vertices.push(end_vertex);
        self.brep.edges.push(Edge::new(0, 0, 1));
    }

    // Dispose
    #[wasm_bindgen]
    pub fn dispose_points(&mut self) {
        self.brep.clear();
    }

    // Destroy and Free memory
    #[wasm_bindgen]
    pub fn destroy(&mut self) {
        self.brep.clear();
        self.id.clear();
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        // Serialize the BREP geometry
        let serialized = serde_json::to_string(&self.brep).unwrap();
        serialized
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        let vertices = self.brep.vertices.clone();
        for vertex in vertices {
            vertex_buffer.push(vertex.position.x);
            vertex_buffer.push(vertex.position.y);
            vertex_buffer.push(vertex.position.z);
        }

        let vertex_serialized = serde_json::to_string(&vertex_buffer).unwrap();
        vertex_serialized
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
        // TODO: Implement DXF serialization for line
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
