use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::spatial::placement::Placement3D;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGLineSlot {
    id: String,
    center: Vector3,
    length: f64,
    width: f64,
    segments: u32,
    placement: Placement3D,
    brep: Brep,
}

#[wasm_bindgen]
impl OGLineSlot {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGLineSlot {
        let internal_id = Uuid::new_v4();

        OGLineSlot {
            id,
            center: Vector3::new(0.0, 0.0, 0.0),
            length: 2.0,
            width: 1.0,
            segments: 32,
            placement: Placement3D::new(),
            brep: Brep::new(internal_id),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(
        &mut self,
        center: Vector3,
        length: f64,
        width: f64,
        segments: u32,
    ) -> Result<(), JsValue> {
        Self::validate_dimensions(length, width).map_err(|err| JsValue::from_str(&err))?;
        self.center = center;
        self.length = length;
        self.width = width;
        self.segments = segments.max(3);
        self.placement.set_anchor(self.center);
        self.generate_geometry()
    }

    #[wasm_bindgen]
    pub fn set_center(&mut self, center: Vector3) {
        self.center = center;
        self.placement.set_anchor(self.center);
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
        let points = self.get_local_points();

        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&points);

        let indices: Vec<u32> = (0..points.len() as u32).collect();
        builder.add_face(&indices, &[]).map_err(|err| {
            JsValue::from_str(&format!("Failed to build line slot face: {}", err))
        })?;

        self.brep = builder.build().map_err(|err| {
            JsValue::from_str(&format!("Failed to finalize line slot BREP: {}", err))
        })?;

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
        serde_json::to_string(&self.world_geometry_buffer()).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_local_geometry_serialized(&self) -> String {
        serde_json::to_string(&self.local_geometry_buffer()).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_buffer(&self) -> Vec<f64> {
        self.world_geometry_buffer()
    }

    #[wasm_bindgen]
    pub fn get_local_geometry_buffer(&self) -> Vec<f64> {
        self.local_geometry_buffer()
    }

    #[wasm_bindgen]
    pub fn get_anchor(&self) -> Vector3 {
        self.placement.anchor
    }
}

impl OGLineSlot {
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

    fn validate_dimensions(length: f64, width: f64) -> Result<(), String> {
        if !length.is_finite() || !width.is_finite() {
            return Err("Line slot length and width must be finite numbers".to_string());
        }
        if width <= 0.0 {
            return Err("Line slot width must be greater than zero".to_string());
        }
        if length <= width {
            return Err("Line slot length must be greater than its width".to_string());
        }
        Ok(())
    }

    // `length` is the overall tip-to-tip length. The right cap sweeps -90deg..90deg and the
    // left cap 90deg..270deg, so the straight edges are the implicit segments joining the
    // caps and no vertex is duplicated.
    pub fn get_local_points(&self) -> Vec<Vector3> {
        let radius = self.width / 2.0;
        let half_span = (self.length - self.width) / 2.0;
        let segments = self.segments.max(3);

        let mut points = Vec::with_capacity(2 * (segments as usize + 1));
        for (cap_x, start_angle) in [(half_span, -FRAC_PI_2), (-half_span, FRAC_PI_2)] {
            let angle_step = PI / segments as f64;
            let mut angle = start_angle;
            for _ in 0..=segments {
                let x = cap_x + radius * angle.cos();
                let y = 0.0;
                let z = radius * angle.sin();
                points.push(Vector3::new(x, y, z));
                angle += angle_step;
            }
        }

        points
    }

    fn local_geometry_buffer(&self) -> Vec<f64> {
        face_loop_buffer(&self.brep)
    }

    fn world_geometry_buffer(&self) -> Vec<f64> {
        face_loop_buffer(&self.world_brep())
    }
}

fn face_loop_buffer(brep: &Brep) -> Vec<f64> {
    let mut vertex_buffer = Vec::new();

    if let Some(face) = brep.faces.first() {
        let mut vertices = brep.get_vertices_by_face_id(face.id);
        if let Some(first) = vertices.first().copied() {
            vertices.push(first);
        }

        for vertex in vertices {
            vertex_buffer.push(vertex.x);
            vertex_buffer.push(vertex.y);
            vertex_buffer.push(vertex.z);
        }
    }

    vertex_buffer
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

    fn configured(length: f64, width: f64, segments: u32) -> OGLineSlot {
        let mut slot = OGLineSlot::new("slot-test".to_string());
        slot.set_config(Vector3::new(0.0, 0.0, 0.0), length, width, segments)
            .expect("line slot config");
        slot
    }

    fn signed_area_xz(points: &[Vector3]) -> f64 {
        let mut sum = 0.0;
        for index in 0..points.len() {
            let current = &points[index];
            let next = &points[(index + 1) % points.len()];
            sum += current.x * next.z - next.x * current.z;
        }
        sum / 2.0
    }

    #[test]
    fn builds_single_face_with_two_caps_of_segment_points() {
        let slot = configured(4.0, 1.0, 8);
        assert_eq!(slot.get_local_points().len(), 18);
        assert_eq!(slot.brep().vertices.len(), 18);
        assert_eq!(slot.brep().faces.len(), 1);
    }

    #[test]
    fn outline_stays_in_xz_plane_within_slot_bounds() {
        let slot = configured(4.0, 1.0, 8);
        let points = slot.get_local_points();

        let mut max_x = f64::MIN;
        for point in &points {
            assert_close(point.y, 0.0);
            assert!(point.z.abs() <= 0.5 + 1.0e-9);
            max_x = max_x.max(point.x);
        }
        assert_close(max_x, 2.0);
    }

    #[test]
    fn cap_points_sit_on_cap_radius() {
        let slot = configured(4.0, 1.0, 8);
        let points = slot.get_local_points();
        let (right_cap, left_cap) = points.split_at(9);

        for point in right_cap {
            assert_close(((point.x - 1.5).powi(2) + point.z.powi(2)).sqrt(), 0.5);
        }
        for point in left_cap {
            assert_close(((point.x + 1.5).powi(2) + point.z.powi(2)).sqrt(), 0.5);
        }
    }

    #[test]
    fn outline_winds_like_rectangle_and_area_matches_slot() {
        let slot = configured(4.0, 1.0, 64);
        let area = signed_area_xz(&slot.get_local_points());

        let expected = PI * 0.5 * 0.5 + (4.0 - 1.0) * 1.0;
        assert!(area > 0.0);
        assert!((area - expected).abs() / expected < 1.0e-3);
    }

    #[test]
    fn rejects_invalid_dimensions() {
        assert!(OGLineSlot::validate_dimensions(2.0, 1.0).is_ok());
        assert!(OGLineSlot::validate_dimensions(2.0, 0.0).is_err());
        assert!(OGLineSlot::validate_dimensions(2.0, -1.0).is_err());
        assert!(OGLineSlot::validate_dimensions(-2.0, 1.0).is_err());
        assert!(OGLineSlot::validate_dimensions(1.0, 1.0).is_err());
        assert!(OGLineSlot::validate_dimensions(0.5, 1.0).is_err());
        assert!(OGLineSlot::validate_dimensions(f64::NAN, 1.0).is_err());
        assert!(OGLineSlot::validate_dimensions(2.0, f64::INFINITY).is_err());
    }

    #[test]
    fn segments_below_three_are_clamped_to_three() {
        let slot = configured(4.0, 1.0, 0);
        assert_eq!(slot.get_local_points().len(), 8);
    }

    #[test]
    fn anchor_follows_center() {
        let mut slot = OGLineSlot::new("slot-anchor".to_string());
        slot.set_config(Vector3::new(3.0, 0.0, 2.0), 4.0, 1.0, 8)
            .expect("line slot config");
        let anchor = slot.get_anchor();
        assert_close(anchor.x, 3.0);
        assert_close(anchor.z, 2.0);
    }
}
