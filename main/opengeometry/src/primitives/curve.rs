/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Curve Primitive for OpenGeometry.
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
pub struct OGCurve {
    id: String,
    control_points: Vec<Vector3>,
    placement: Placement3D,
    brep: Brep,
    anchor_initialized: bool,
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
            placement: Placement3D::new(),
            brep: Brep::new(Uuid::new_v4()),
            anchor_initialized: false,
        }
    }

    #[wasm_bindgen]
    pub fn set_config(&mut self, control_points: Vec<Vector3>) -> Result<(), JsValue> {
        self.control_points = control_points;
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
        if self.control_points.is_empty() {
            self.brep.clear();
            return Ok(());
        }

        self.brep.clear();

        let local_points = self.local_points();
        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&local_points);

        if local_points.len() >= 2 {
            let indices: Vec<u32> = (0..local_points.len() as u32).collect();
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

    pub fn world_brep(&self) -> Brep {
        self.brep.transformed(&self.placement)
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
        let points = transform_points_with_placement(&self.control_points, &self.placement);
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

    fn local_points(&self) -> Vec<Vector3> {
        points_relative_to_anchor(&self.control_points, self.placement.anchor)
    }

    fn recompute_anchor_from_bounds(&mut self) {
        let anchor =
            bounds_center_from_points(&self.control_points).unwrap_or(Vector3::new(0.0, 0.0, 0.0));
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
        return brep
            .vertices
            .iter()
            .flat_map(|vertex| [vertex.position.x, vertex.position.y, vertex.position.z])
            .collect();
    };

    brep.get_wire_vertex_buffer(wire.id, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_stays_stable_across_curve_config_updates_until_reset() {
        let mut curve = OGCurve::new("curve-anchor".to_string());
        curve
            .set_config(vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(10.0, 0.0, 0.0),
            ])
            .expect("curve config");
        let initial_anchor = curve.get_anchor();
        assert_eq!(initial_anchor.x, 5.0);

        curve
            .set_config(vec![
                Vector3::new(10.0, 0.0, 0.0),
                Vector3::new(20.0, 0.0, 0.0),
            ])
            .expect("curve config update");
        let anchor_after_update = curve.get_anchor();
        assert_eq!(anchor_after_update.x, 5.0);

        curve.reset_anchor().expect("reset anchor");
        let anchor_after_reset = curve.get_anchor();
        assert_eq!(anchor_after_reset.x, 15.0);
    }
}
