use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::Brep;
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::sweep::{sweep_profile_along_path, SweepOptions};
use crate::primitives::line::OGLine;
use crate::primitives::polyline::OGPolyline;
use crate::primitives::rectangle::OGRectangle;
use crate::spatial::placement::{
    bounds_center_from_point_sets, points_relative_to_anchor, Placement3D,
};
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGSweep {
    id: String,
    path_points: Vec<Vector3>,
    profile_points: Vec<Vector3>,
    cap_start: bool,
    cap_end: bool,
    placement: Placement3D,
    brep: Brep,
    anchor_initialized: bool,
}

#[wasm_bindgen]
impl OGSweep {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGSweep {
        let internal_id = Uuid::new_v4();

        OGSweep {
            id,
            path_points: Vec::new(),
            profile_points: Vec::new(),
            cap_start: true,
            cap_end: true,
            placement: Placement3D::new(),
            brep: Brep::new(internal_id),
            anchor_initialized: false,
        }
    }

    #[wasm_bindgen]
    pub fn set_config(
        &mut self,
        path_points: Vec<Vector3>,
        profile_points: Vec<Vector3>,
    ) -> Result<(), JsValue> {
        self.path_points = path_points;
        self.profile_points = profile_points;
        self.cap_start = true;
        self.cap_end = true;
        self.ensure_anchor_initialized();
        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn set_config_with_caps(
        &mut self,
        path_points: Vec<Vector3>,
        profile_points: Vec<Vector3>,
        cap_start: bool,
        cap_end: bool,
    ) -> Result<(), JsValue> {
        self.path_points = path_points;
        self.profile_points = profile_points;
        self.cap_start = cap_start;
        self.cap_end = cap_end;
        self.ensure_anchor_initialized();
        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn set_caps(&mut self, cap_start: bool, cap_end: bool) -> Result<(), JsValue> {
        self.cap_start = cap_start;
        self.cap_end = cap_end;
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

    pub fn generate_brep(&mut self) -> Result<(), JsValue> {
        self.clean_geometry();
        self.generate_geometry()
    }

    pub fn clean_geometry(&mut self) {
        self.brep.clear();
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        let options = SweepOptions {
            cap_start: self.cap_start,
            cap_end: self.cap_end,
        };

        let local_path_points = points_relative_to_anchor(&self.path_points, self.placement.anchor);
        let local_profile_points =
            points_relative_to_anchor(&self.profile_points, self.placement.anchor);

        self.brep = sweep_profile_along_path(&local_path_points, &local_profile_points, options)
            .map_err(|err| JsValue::from_str(&format!("Sweep generation failed: {}", err)))?;
        self.brep
            .validate_topology()
            .map_err(|err| JsValue::from_str(&format!("Invalid sweep topology: {}", err)))?;

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

impl OGSweep {
    pub fn set_path_from_polyline(&mut self, polyline: &OGPolyline) -> Result<(), JsValue> {
        self.path_points = wire_points_from_brep(&polyline.world_brep());
        self.ensure_anchor_initialized();
        self.generate_brep()
    }

    pub fn set_path_from_line(&mut self, line: &OGLine) -> Result<(), JsValue> {
        self.path_points = wire_points_from_brep(&line.world_brep());
        self.ensure_anchor_initialized();
        self.generate_brep()
    }

    pub fn set_profile_from_polyline(&mut self, profile: &OGPolyline) -> Result<(), JsValue> {
        self.profile_points = wire_points_from_brep(&profile.world_brep());
        self.ensure_anchor_initialized();
        self.generate_brep()
    }

    pub fn set_profile_from_rectangle(&mut self, rectangle: &OGRectangle) -> Result<(), JsValue> {
        self.profile_points = face_points_from_brep(&rectangle.world_brep());
        self.ensure_anchor_initialized();
        self.generate_brep()
    }

    pub fn path_points(&self) -> Vec<Vector3> {
        self.path_points.clone()
    }

    pub fn profile_points(&self) -> Vec<Vector3> {
        self.profile_points.clone()
    }

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

    fn recompute_anchor_from_bounds(&mut self) {
        let point_sets = [self.path_points.as_slice(), self.profile_points.as_slice()];
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

fn wire_points_from_brep(brep: &Brep) -> Vec<Vector3> {
    let Some(wire) = brep.wires.first() else {
        return Vec::new();
    };

    brep.get_wire_vertex_indices(wire.id)
        .into_iter()
        .filter_map(|vertex_id| brep.vertices.get(vertex_id as usize))
        .map(|vertex| vertex.position)
        .collect()
}

fn face_points_from_brep(brep: &Brep) -> Vec<Vector3> {
    let Some(face) = brep.faces.first() else {
        return Vec::new();
    };

    brep.get_vertices_by_face_id(face.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_stays_stable_across_sweep_config_updates_until_reset() {
        let mut sweep = OGSweep::new("sweep-anchor".to_string());
        sweep
            .set_config(
                vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0)],
                vec![
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 1.0),
                    Vector3::new(0.0, 1.0, 1.0),
                    Vector3::new(0.0, 1.0, 0.0),
                ],
            )
            .expect("sweep config");
        let initial_anchor = sweep.get_anchor();
        assert_eq!(initial_anchor.x, 5.0);

        sweep
            .set_config(
                vec![Vector3::new(10.0, 0.0, 0.0), Vector3::new(20.0, 0.0, 0.0)],
                vec![
                    Vector3::new(10.0, 0.0, 0.0),
                    Vector3::new(10.0, 0.0, 1.0),
                    Vector3::new(10.0, 1.0, 1.0),
                    Vector3::new(10.0, 1.0, 0.0),
                ],
            )
            .expect("sweep config update");
        let anchor_after_update = sweep.get_anchor();
        assert_eq!(anchor_after_update.x, 5.0);

        sweep.reset_anchor().expect("reset anchor");
        let anchor_after_reset = sweep.get_anchor();
        assert_eq!(anchor_after_reset.x, 15.0);
    }
}
