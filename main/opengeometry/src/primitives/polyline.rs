/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Polyline Primitive for OpenGeometry.
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
pub struct OGPolyline {
    id: String,
    points: Vec<Vector3>,
    is_closed: bool,
    brep: Brep,
}

impl OGPolyline {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}

impl Drop for OGPolyline {
    fn drop(&mut self) {
        self.points.clear();
        self.is_closed = false;
        self.brep.clear();
        self.id.clear();
    }
}

#[wasm_bindgen]
impl OGPolyline {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGPolyline {
        OGPolyline {
            id,
            points: Vec::new(),
            is_closed: false,
            brep: Brep::new(Uuid::new_v4()),
        }
    }

    #[wasm_bindgen]
    pub fn clone(&self) -> OGPolyline {
        OGPolyline {
            id: self.id.clone(),
            points: self.points.clone(),
            is_closed: self.is_closed,
            brep: self.brep.clone(),
        }
    }

    pub fn set_config(&mut self, points: Vec<Vector3>) -> Result<(), JsValue> {
        self.points = points;
        self.check_closed_test();
        self.generate_geometry()
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        if self.points.is_empty() {
            self.brep.clear();
            return Ok(());
        }

        let mut effective_points = self.points.clone();
        if self.is_closed && effective_points.len() > 2 {
            let first = effective_points[0];
            let last = *effective_points.last().unwrap();
            let dx = first.x - last.x;
            let dy = first.y - last.y;
            let dz = first.z - last.z;
            let duplicate_end = dx * dx + dy * dy + dz * dz <= 1.0e-12;
            if duplicate_end {
                effective_points.pop();
            }
        }

        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&effective_points);

        if effective_points.len() >= 2 {
            let indices: Vec<u32> = (0..effective_points.len() as u32).collect();
            let closed_wire = self.is_closed && effective_points.len() > 2;
            builder.add_wire(&indices, closed_wire).map_err(|err| {
                JsValue::from_str(&format!("Failed to build polyline wire: {}", err))
            })?;

            if closed_wire {
                // The wire already consumes directed edges in index order.
                // Reversing for the face loop avoids duplicate-directed-halfedge collisions
                // while still building a valid face with twin halfedges.
                let mut face_indices = indices.clone();
                face_indices.reverse();

                builder.add_face(&face_indices, &[]).map_err(|err| {
                    JsValue::from_str(&format!("Failed to build polyline face: {}", err))
                })?;
            }
        }

        self.brep = builder.build().map_err(|err| {
            JsValue::from_str(&format!("Failed to finalize polyline BREP: {}", err))
        })?;

        Ok(())
    }

    #[wasm_bindgen]
    pub fn add_multiple_points(&mut self, points: Vec<Vector3>) -> Result<(), JsValue> {
        self.points = points;
        self.check_closed_test();
        self.generate_geometry()
    }

    #[wasm_bindgen]
    pub fn add_point(&mut self, point: Vector3) -> Result<(), JsValue> {
        self.points.push(point);
        self.check_closed_test();
        self.generate_geometry()
    }

    #[wasm_bindgen]
    pub fn get_points(&self) -> String {
        serde_json::to_string(&self.points).unwrap()
    }

    pub fn get_raw_points(&self) -> Vec<Vector3> {
        self.points.clone()
    }

    #[wasm_bindgen]
    pub fn is_closed(&self) -> bool {
        self.is_closed
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

    pub fn check_closed_test(&mut self) {
        self.is_closed = false;
        if self.points.len() > 2 {
            let first = self.points[0];
            let last = self.points[self.points.len() - 1];
            if first.x == last.x && first.y == last.y && first.z == last.z {
                self.is_closed = true;
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

        if let Some(wire) = self.brep.wires.first() {
            let wire_vertices = self.brep.get_wire_vertex_indices(wire.id);
            for vertex_id in &wire_vertices {
                if let Some(vertex) = self.brep.vertices.get(*vertex_id as usize) {
                    vertex_buffer.push(vertex.position.x);
                    vertex_buffer.push(vertex.position.y);
                    vertex_buffer.push(vertex.position.z);
                }
            }

            if self.is_closed && !wire_vertices.is_empty() {
                if let Some(first_id) = wire_vertices.first() {
                    if let Some(first_vertex) = self.brep.vertices.get(*first_id as usize) {
                        vertex_buffer.push(first_vertex.position.x);
                        vertex_buffer.push(first_vertex.position.y);
                        vertex_buffer.push(first_vertex.position.z);
                    }
                }
            }
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }
}

impl OGPolyline {
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
        offset_path(&self.points, distance, Some(self.is_closed), options)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_polyline_builds_wire_and_face_without_duplicate_halfedge_error() {
        let mut polyline = OGPolyline::new("polyline-test".to_string());
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 2.0),
            Vector3::new(0.0, 0.0, 2.0),
            Vector3::new(0.0, 0.0, 0.0),
        ];

        polyline
            .set_config(points)
            .expect("closed polyline should build without duplicate directed halfedge");

        assert!(polyline.is_closed());
        assert_eq!(polyline.brep.wires.len(), 1);
        assert_eq!(polyline.brep.faces.len(), 1);
    }
}
