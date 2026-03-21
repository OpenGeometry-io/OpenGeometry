use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::spatial::placement::{
    bounds_center_from_point_sets, points_relative_to_anchor, Placement3D,
};
use crate::utility::bgeometry::BufferGeometry;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGPolygon {
    id: String,
    points: Vec<Vector3>,
    holes: Vec<Vec<Vector3>>,
    placement: Placement3D,
    geometry: BufferGeometry,
    brep: Brep,
    anchor_initialized: bool,
}

impl OGPolygon {
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

#[wasm_bindgen]
impl OGPolygon {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGPolygon {
        let internal_id = Uuid::new_v4();

        OGPolygon {
            id,
            points: Vec::new(),
            holes: Vec::new(),
            placement: Placement3D::new(),
            geometry: BufferGeometry::new(internal_id),
            brep: Brep::new(internal_id),
            anchor_initialized: false,
        }
    }

    #[wasm_bindgen]
    pub fn set_config(&mut self, points: Vec<Vector3>) -> Result<(), JsValue> {
        self.points = points;
        self.holes.clear();
        self.ensure_anchor_initialized();
        self.generate_brep()
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
    pub fn add_vertices(&mut self, vertices: Vec<Vector3>) -> Result<(), JsValue> {
        self.points = vertices;
        self.holes.clear();
        self.ensure_anchor_initialized();
        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn add_holes(&mut self, hole: Vec<Vector3>) -> Result<(), JsValue> {
        self.holes.push(hole);
        self.ensure_anchor_initialized();
        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn clean_geometry(&mut self) {
        self.brep.clear();
        self.geometry.clear();
    }

    #[wasm_bindgen]
    pub fn generate_brep(&mut self) -> Result<(), JsValue> {
        if self.points.len() < 3 {
            self.clean_geometry();
            return Ok(());
        }

        self.clean_geometry();

        let mut builder = BrepBuilder::new(self.brep.id);

        let anchor = self.placement.anchor;
        let local_points = points_relative_to_anchor(&self.points, anchor);
        let mut all_vertices = local_points.clone();
        let mut hole_index_sets: Vec<Vec<u32>> = Vec::new();

        for hole in &self.holes {
            if hole.len() < 3 {
                continue;
            }

            let start = all_vertices.len() as u32;
            let local_hole = points_relative_to_anchor(hole, anchor);
            all_vertices.extend(local_hole);
            let indices: Vec<u32> = (0..hole.len() as u32)
                .map(|offset| start + offset)
                .collect();
            hole_index_sets.push(indices);
        }

        builder.add_vertices(&all_vertices);
        let outer_indices: Vec<u32> = (0..local_points.len() as u32).collect();

        builder
            .add_face(&outer_indices, &hole_index_sets)
            .map_err(|err| JsValue::from_str(&format!("Failed to build polygon face: {}", err)))?;

        self.brep = builder.build().map_err(|err| {
            JsValue::from_str(&format!("Failed to finalize polygon BREP: {}", err))
        })?;

        Ok(())
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        self.generate_brep()
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

    #[wasm_bindgen]
    pub fn set_anchor(&mut self, anchor: Vector3) -> Result<(), JsValue> {
        self.placement.set_anchor(anchor);
        self.anchor_initialized = true;
        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn reset_anchor(&mut self) -> Result<(), JsValue> {
        self.recompute_anchor_from_bounds();
        self.anchor_initialized = true;
        self.generate_brep()
    }
}

impl OGPolygon {
    fn recompute_anchor_from_bounds(&mut self) {
        let mut point_sets = Vec::with_capacity(self.holes.len() + 1);
        point_sets.push(self.points.as_slice());
        for hole in &self.holes {
            point_sets.push(hole.as_slice());
        }

        let anchor =
            bounds_center_from_point_sets(&point_sets).unwrap_or(Vector3::new(0.0, 0.0, 0.0));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_stays_stable_across_polygon_config_updates_until_reset() {
        let mut polygon = OGPolygon::new("polygon-anchor".to_string());
        polygon
            .set_config(vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(10.0, 0.0, 0.0),
                Vector3::new(10.0, 0.0, 10.0),
                Vector3::new(0.0, 0.0, 10.0),
            ])
            .expect("polygon config");
        let initial_anchor = polygon.get_anchor();
        assert_eq!(initial_anchor.x, 5.0);
        assert_eq!(initial_anchor.z, 5.0);

        polygon
            .set_config(vec![
                Vector3::new(10.0, 0.0, 10.0),
                Vector3::new(20.0, 0.0, 10.0),
                Vector3::new(20.0, 0.0, 20.0),
                Vector3::new(10.0, 0.0, 20.0),
            ])
            .expect("polygon config update");
        let anchor_after_update = polygon.get_anchor();
        assert_eq!(anchor_after_update.x, 5.0);
        assert_eq!(anchor_after_update.z, 5.0);

        polygon.reset_anchor().expect("reset anchor");
        let anchor_after_reset = polygon.get_anchor();
        assert_eq!(anchor_after_reset.x, 15.0);
        assert_eq!(anchor_after_reset.z, 15.0);
    }

    #[test]
    fn placement_rejects_non_uniform_scale() {
        let mut polygon = OGPolygon::new("polygon-scale".to_string());
        polygon
            .set_config(vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 1.0),
            ])
            .expect("polygon config");

        let result = polygon.placement.set_transform(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.2, 1.0),
        );
        assert!(result.is_err());
    }
}
