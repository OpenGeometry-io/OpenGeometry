/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Line Primitive for OpenGeometry.
 */
use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::offset::{offset_path, OffsetOptions, OffsetResult};
use crate::spatial::placement::{
    bounds_center_from_points, points_relative_to_anchor, Placement3D,
};
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
    placement: Placement3D,
    anchor_initialized: bool,
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
            placement: Placement3D::new(),
            anchor_initialized: false,
        }
    }

    #[wasm_bindgen]
    pub fn set_config(&mut self, start: Vector3, end: Vector3) -> Result<(), JsValue> {
        self.start = start;
        self.end = end;
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
        self.brep.clear();

        let local_points = self.local_points();
        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&local_points);
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

    pub fn world_brep(&self) -> Brep {
        self.brep.transformed(&self.placement)
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
        let points = self.world_points();
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
        let world_brep = self.world_brep();
        project_brep_to_scene(&world_brep, camera, hlr)
    }

    pub fn world_points(&self) -> Vec<Vector3> {
        let world_brep = self.world_brep();
        world_brep
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect()
    }

    fn local_points(&self) -> Vec<Vector3> {
        let source_points = vec![self.start, self.end];
        let anchor = self.placement.anchor;
        points_relative_to_anchor(&source_points, anchor)
    }

    fn recompute_anchor_from_bounds(&mut self) {
        let source_points = vec![self.start, self.end];
        let anchor =
            bounds_center_from_points(&source_points).unwrap_or(Vector3::new(0.0, 0.0, 0.0));
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

    brep.get_wire_vertex_buffer(wire.id, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        let delta = (actual - expected).abs();
        assert!(
            delta <= 1.0e-9,
            "expected {expected}, got {actual}, delta {delta}"
        );
    }

    #[test]
    fn anchor_stays_stable_across_config_updates_until_reset() {
        let mut line = OGLine::new("line-anchor".to_string());
        line.set_config(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0))
            .expect("line config");
        let initial_anchor = line.get_anchor();
        assert_close(initial_anchor.x, 5.0);

        line.set_config(Vector3::new(10.0, 0.0, 0.0), Vector3::new(20.0, 0.0, 0.0))
            .expect("line config update");
        let anchor_after_update = line.get_anchor();
        assert_close(anchor_after_update.x, 5.0);

        line.reset_anchor().expect("reset anchor");
        let anchor_after_reset = line.get_anchor();
        assert_close(anchor_after_reset.x, 15.0);
    }
}
