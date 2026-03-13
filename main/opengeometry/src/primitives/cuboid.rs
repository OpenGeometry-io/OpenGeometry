use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
use crate::operations::triangulate::triangulate_polygon_with_holes;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGCuboid {
    id: String,
    center: Vector3,
    width: f64,
    height: f64,
    depth: f64,
    brep: Brep,
}

#[wasm_bindgen]
impl OGCuboid {
    #[wasm_bindgen(setter)]
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> OGCuboid {
        let internal_id = Uuid::new_v4();

        OGCuboid {
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
        let hw = self.width / 2.0;
        let hh = self.height / 2.0;
        let hd = self.depth / 2.0;

        let vertices = vec![
            Vector3::new(self.center.x - hw, self.center.y - hh, self.center.z - hd),
            Vector3::new(self.center.x + hw, self.center.y - hh, self.center.z - hd),
            Vector3::new(self.center.x + hw, self.center.y - hh, self.center.z + hd),
            Vector3::new(self.center.x - hw, self.center.y - hh, self.center.z + hd),
            Vector3::new(self.center.x - hw, self.center.y + hh, self.center.z - hd),
            Vector3::new(self.center.x + hw, self.center.y + hh, self.center.z - hd),
            Vector3::new(self.center.x + hw, self.center.y + hh, self.center.z + hd),
            Vector3::new(self.center.x - hw, self.center.y + hh, self.center.z + hd),
        ];

        let faces: Vec<Vec<u32>> = vec![
            vec![0, 1, 2, 3],
            vec![4, 7, 6, 5],
            vec![0, 4, 5, 1],
            vec![3, 2, 6, 7],
            vec![0, 3, 7, 4],
            vec![1, 5, 6, 2],
        ];

        let mut builder = BrepBuilder::new(self.brep.id);
        builder.add_vertices(&vertices);

        for face in &faces {
            builder.add_face(face, &[]).map_err(|err| {
                JsValue::from_str(&format!("Failed to build cuboid face: {}", err))
            })?;
        }

        builder
            .add_shell_from_all_faces(true)
            .map_err(|err| JsValue::from_str(&format!("Failed to build cuboid shell: {}", err)))?;

        self.brep = builder.build().map_err(|err| {
            JsValue::from_str(&format!("Failed to finalize cuboid BREP: {}", err))
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

impl OGCuboid {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
