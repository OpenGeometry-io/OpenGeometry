use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::spatial::placement::Placement3D;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGWedge {
    id: String,
    center: Vector3,
    width: f64,
    height: f64,
    depth: f64,
    placement: Placement3D,
    brep: Brep,
}

#[wasm_bindgen]
impl OGWedge {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGWedge {
        let internal_id = Uuid::new_v4();

        OGWedge {
            id,
            center: Vector3::new(0.0, 0.0, 0.0),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
            placement: Placement3D::new(),
            brep: Brep::new(internal_id),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(
        &mut self,
        center: Vector3,
        width: f64,
        height: f64,
        depth: f64,
    ) -> Result<(), JsValue> {
        self.center = center;
        self.width = width;
        self.height = height;
        self.depth = depth;
        self.placement.set_anchor(self.center);
        self.generate_brep()
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

    pub fn generate_brep(&mut self) -> Result<(), JsValue> {
        self.clean_geometry();
        self.generate_geometry()
    }

    pub fn clean_geometry(&mut self) {
        self.brep.clear();
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        let half_width = self.width / 2.0;
        let half_height = self.height / 2.0;
        let half_depth = self.depth / 2.0;

        let x_min = -half_width;
        let x_max = half_width;
        let y_min = -half_height;
        let y_max = half_height;
        let z_min = -half_depth;
        let z_max = half_depth;

        let vertices = vec![
            Vector3::new(x_min, y_min, z_min),
            Vector3::new(x_max, y_min, z_min),
            Vector3::new(x_min, y_max, z_min),
            Vector3::new(x_min, y_min, z_max),
            Vector3::new(x_max, y_min, z_max),
            Vector3::new(x_min, y_max, z_max),
        ];

        let faces = vec![
            vec![0, 1, 4, 3],
            vec![0, 2, 1],
            vec![3, 4, 5],
            vec![0, 3, 5, 2],
            vec![1, 2, 5, 4],
        ];

        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&vertices);

        for face in &faces {
            builder.add_face(face, &[]).map_err(|err| {
                JsValue::from_str(&format!("Failed to build wedge face: {}", err))
            })?;
        }

        builder
            .add_shell_from_all_faces(true)
            .map_err(|err| JsValue::from_str(&format!("Failed to build wedge shell: {}", err)))?;

        self.brep = builder
            .build()
            .map_err(|err| JsValue::from_str(&format!("Failed to finalize wedge BREP: {}", err)))?;

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
}

impl OGWedge {
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
}
