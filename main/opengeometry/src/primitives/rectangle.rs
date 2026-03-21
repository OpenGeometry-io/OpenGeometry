use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::offset::{offset_path, OffsetOptions, OffsetResult};
use crate::spatial::placement::Placement3D;
use crate::utility::bgeometry::BufferGeometry;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGRectangle {
    id: String,
    center: Vector3,
    width: f64,
    breadth: f64,
    placement: Placement3D,
    geometry: BufferGeometry,
    brep: Brep,
}

#[wasm_bindgen]
impl OGRectangle {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGRectangle {
        let internal_id = Uuid::new_v4();

        OGRectangle {
            id,
            center: Vector3::new(0.0, 0.0, 0.0),
            width: 1.0,
            breadth: 1.0,
            placement: Placement3D::new(),
            geometry: BufferGeometry::new(internal_id),
            brep: Brep::new(internal_id),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(&mut self, center: Vector3, width: f64, breadth: f64) -> Result<(), JsValue> {
        self.center = center;
        self.width = width;
        self.breadth = breadth;
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

        let indices = vec![0, 1, 2, 3];
        builder.add_face(&indices, &[]).map_err(|err| {
            JsValue::from_str(&format!("Failed to build rectangle face: {}", err))
        })?;

        self.brep = builder.build().map_err(|err| {
            JsValue::from_str(&format!("Failed to finalize rectangle BREP: {}", err))
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
}

impl OGRectangle {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn get_raw_points(&self) -> Vec<Vector3> {
        let half_width = self.width / 2.0;
        let half_breadth = self.breadth / 2.0;
        let center = self.center;

        vec![
            Vector3::new(-half_width, 0.0, -half_breadth).add(&center),
            Vector3::new(half_width, 0.0, -half_breadth).add(&center),
            Vector3::new(half_width, 0.0, half_breadth).add(&center),
            Vector3::new(-half_width, 0.0, half_breadth).add(&center),
        ]
    }

    pub fn get_local_points(&self) -> Vec<Vector3> {
        let half_width = self.width / 2.0;
        let half_breadth = self.breadth / 2.0;

        vec![
            Vector3::new(-half_width, 0.0, -half_breadth),
            Vector3::new(half_width, 0.0, -half_breadth),
            Vector3::new(half_width, 0.0, half_breadth),
            Vector3::new(-half_width, 0.0, half_breadth),
        ]
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
        let points = self.world_face_points();
        offset_path(&points, distance, Some(true), options)
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

    pub fn world_brep(&self) -> Brep {
        self.brep.transformed(&self.placement)
    }

    fn local_geometry_buffer(&self) -> Vec<f64> {
        face_loop_buffer(&self.brep)
    }

    fn world_geometry_buffer(&self) -> Vec<f64> {
        face_loop_buffer(&self.world_brep())
    }

    fn world_face_points(&self) -> Vec<Vector3> {
        let world_brep = self.world_brep();
        if let Some(face) = world_brep.faces.first() {
            return world_brep.get_vertices_by_face_id(face.id);
        }

        Vec::new()
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
