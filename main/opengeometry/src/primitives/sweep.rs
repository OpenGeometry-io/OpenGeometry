use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::Brep;
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::sweep::{sweep_profile_along_path, SweepOptions};
use crate::operations::triangulate::triangulate_polygon_with_holes;
use crate::primitives::line::OGLine;
use crate::primitives::polyline::OGPolyline;
use crate::primitives::rectangle::OGRectangle;
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
    brep: Brep,
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
            brep: Brep::new(internal_id),
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
        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn set_caps(&mut self, cap_start: bool, cap_end: bool) -> Result<(), JsValue> {
        self.cap_start = cap_start;
        self.cap_end = cap_end;
        self.generate_brep()
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

        self.brep = sweep_profile_along_path(&self.path_points, &self.profile_points, options)
            .map_err(|err| JsValue::from_str(&format!("Sweep generation failed: {}", err)))?;
        self.brep
            .validate_topology()
            .map_err(|err| JsValue::from_str(&format!("Invalid sweep topology: {}", err)))?;

        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        for face in &self.brep.faces {
            let (face_vertices, holes_vertices) =
                self.brep.get_vertices_and_holes_by_face_id(face.id);

            if face_vertices.len() < 3 {
                continue;
            }

            let triangles = triangulate_polygon_with_holes(&face_vertices, &holes_vertices);
            let all_vertices: Vec<Vector3> = face_vertices
                .into_iter()
                .chain(holes_vertices.into_iter().flatten())
                .collect();

            for triangle in triangles {
                for vertex_index in triangle {
                    let vertex = &all_vertices[vertex_index];
                    vertex_buffer.push(vertex.x);
                    vertex_buffer.push(vertex.y);
                    vertex_buffer.push(vertex.z);
                }
            }
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_outline_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();

        for (start_id, end_id) in self.brep.collect_outline_segments() {
            let Some(start_vertex) = self.brep.vertices.get(start_id as usize) else {
                continue;
            };
            let Some(end_vertex) = self.brep.vertices.get(end_id as usize) else {
                continue;
            };

            vertex_buffer.push(start_vertex.position.x);
            vertex_buffer.push(start_vertex.position.y);
            vertex_buffer.push(start_vertex.position.z);
            vertex_buffer.push(end_vertex.position.x);
            vertex_buffer.push(end_vertex.position.y);
            vertex_buffer.push(end_vertex.position.z);
        }

        serde_json::to_string(&vertex_buffer).unwrap()
    }
}

impl OGSweep {
    pub fn set_path_from_polyline(&mut self, polyline: &OGPolyline) -> Result<(), JsValue> {
        self.path_points = polyline.get_raw_points();
        self.generate_brep()
    }

    pub fn set_path_from_line(&mut self, line: &OGLine) -> Result<(), JsValue> {
        self.path_points = line
            .brep()
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect();
        self.generate_brep()
    }

    pub fn set_profile_from_polyline(&mut self, profile: &OGPolyline) -> Result<(), JsValue> {
        self.profile_points = profile.get_raw_points();
        self.generate_brep()
    }

    pub fn set_profile_from_rectangle(&mut self, rectangle: &OGRectangle) -> Result<(), JsValue> {
        self.profile_points = rectangle
            .brep()
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect();
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

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
