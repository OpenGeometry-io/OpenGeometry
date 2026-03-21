/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Polyline Primitive for OpenGeometry.
 */
use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::offset::{offset_path, OffsetOptions, OffsetResult};
use crate::spatial::placement::{
    bounds_center_from_points, points_relative_to_anchor, transform_points_with_placement,
    Placement3D,
};
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
    placement: Placement3D,
    brep: Brep,
    anchor_initialized: bool,
}

impl OGPolyline {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        let world_brep = self.world_brep();
        project_brep_to_scene(&world_brep, camera, hlr)
    }

    pub fn world_brep(&self) -> Brep {
        self.brep.transformed(&self.placement)
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
            placement: Placement3D::new(),
            brep: Brep::new(Uuid::new_v4()),
            anchor_initialized: false,
        }
    }

    #[wasm_bindgen]
    pub fn clone(&self) -> OGPolyline {
        OGPolyline {
            id: self.id.clone(),
            points: self.points.clone(),
            is_closed: self.is_closed,
            placement: self.placement.clone(),
            brep: self.brep.clone(),
            anchor_initialized: self.anchor_initialized,
        }
    }

    pub fn set_config(&mut self, points: Vec<Vector3>) -> Result<(), JsValue> {
        self.points = points;
        self.check_closed_test();
        self.ensure_anchor_initialized();
        self.generate_geometry()
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
        if self.points.is_empty() {
            self.brep.clear();
            return Ok(());
        }

        self.brep.clear();

        let mut effective_points = self.local_points();
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
        self.ensure_anchor_initialized();
        self.generate_geometry()
    }

    #[wasm_bindgen]
    pub fn add_point(&mut self, point: Vector3) -> Result<(), JsValue> {
        self.points.push(point);
        self.check_closed_test();
        self.ensure_anchor_initialized();
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

    #[wasm_bindgen]
    pub fn set_anchor(&mut self, anchor: Vector3) -> Result<(), JsValue> {
        self.placement.set_anchor(anchor);
        self.anchor_initialized = true;
        self.generate_geometry()
    }

    #[wasm_bindgen]
    pub fn reset_anchor(&mut self) -> Result<(), JsValue> {
        self.recompute_anchor_from_bounds();
        self.anchor_initialized = true;
        self.generate_geometry()
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
        let points = transform_points_with_placement(&self.points, &self.placement);
        offset_path(&points, distance, Some(self.is_closed), options)
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

    fn local_points(&self) -> Vec<Vector3> {
        points_relative_to_anchor(&self.points, self.placement.anchor)
    }

    fn recompute_anchor_from_bounds(&mut self) {
        let anchor = bounds_center_from_points(&self.points).unwrap_or(Vector3::new(0.0, 0.0, 0.0));
        self.placement.set_anchor(anchor);
    }

    fn ensure_anchor_initialized(&mut self) {
        if self.anchor_initialized {
            return;
        }
        self.recompute_anchor_from_bounds();
        self.anchor_initialized = true;
    }
}

fn wire_geometry_buffer(brep: &Brep) -> Vec<f64> {
    let Some(wire) = brep.wires.first() else {
        return Vec::new();
    };

    brep.get_wire_vertex_buffer(wire.id, wire.is_closed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_polyline_builds_wire_without_duplicate_halfedge_error() {
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
        assert_eq!(polyline.brep.faces.len(), 0);
    }

    #[test]
    fn anchor_stays_stable_across_polyline_config_updates_until_reset() {
        let mut polyline = OGPolyline::new("polyline-anchor".to_string());
        polyline
            .set_config(vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(10.0, 0.0, 0.0),
            ])
            .expect("polyline config");
        let initial_anchor = polyline.get_anchor();
        assert_eq!(initial_anchor.x, 5.0);

        polyline
            .set_config(vec![
                Vector3::new(10.0, 0.0, 0.0),
                Vector3::new(20.0, 0.0, 0.0),
            ])
            .expect("polyline config update");
        let anchor_after_update = polyline.get_anchor();
        assert_eq!(anchor_after_update.x, 5.0);

        polyline.reset_anchor().expect("reset anchor");
        let anchor_after_reset = polyline.get_anchor();
        assert_eq!(anchor_after_reset.x, 15.0);
    }
}
