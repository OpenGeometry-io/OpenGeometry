use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{
    collect_visible_outline_vertex_buffer, project_brep_to_scene, CameraParameters, HlrOptions,
    ProjectionMode, Scene2D,
};
use crate::operations::triangulate::triangulate_polygon_with_holes;
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
        let half_width = self.width / 2.0;
        let half_height = self.height / 2.0;
        let half_depth = self.depth / 2.0;

        let x_min = self.center.x - half_width;
        let x_max = self.center.x + half_width;
        let y_min = self.center.y - half_height;
        let y_max = self.center.y + half_height;
        let z_min = self.center.z - half_depth;
        let z_max = self.center.z + half_depth;

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

    #[wasm_bindgen]
    pub fn get_outline_geometry_hlr_serialized(
        &self,
        camera_position: Vector3,
        camera_target: Vector3,
        camera_up: Vector3,
        near: f64,
        hide_hidden_edges: bool,
    ) -> String {
        let camera = CameraParameters {
            position: camera_position,
            target: camera_target,
            up: camera_up,
            near: near.max(1.0e-6),
            projection_mode: ProjectionMode::Perspective,
        };

        let hlr = HlrOptions { hide_hidden_edges };
        let vertex_buffer = collect_visible_outline_vertex_buffer(&self.brep, &camera, &hlr);
        serde_json::to_string(&vertex_buffer).unwrap()
    }
}

impl OGWedge {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
