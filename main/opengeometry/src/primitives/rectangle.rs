use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::offset::{offset_path, OffsetOptions, OffsetResult};
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
            geometry: BufferGeometry::new(internal_id),
            brep: Brep::new(internal_id),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(&mut self, center: Vector3, width: f64, breadth: f64) -> Result<(), JsValue> {
        self.center = center;
        self.width = width;
        self.breadth = breadth;
        Ok(())
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) -> Result<(), JsValue> {
        let points = self.get_raw_points();

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
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        if let Some(face) = self.brep.faces.first() {
            let mut vertices = self.brep.get_vertices_by_face_id(face.id);
            if let Some(first) = vertices.first().copied() {
                vertices.push(first);
            }

            for vertex in vertices {
                vertex_buffer.push(vertex.x);
                vertex_buffer.push(vertex.y);
                vertex_buffer.push(vertex.z);
            }
        }

        serde_json::to_string(&vertex_buffer).unwrap()
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
        let points = self.get_raw_points();
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
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
