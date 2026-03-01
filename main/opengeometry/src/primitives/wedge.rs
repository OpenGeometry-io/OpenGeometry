use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, Edge, Face, Vertex};
use crate::export::projection::{project_brep_to_scene, CameraParameters, HlrOptions, Scene2D};
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
    pub fn set_config(&mut self, center: Vector3, width: f64, height: f64, depth: f64) {
        self.center = center;
        self.width = width;
        self.height = height;
        self.depth = depth;

        self.generate_brep();
    }

    pub fn generate_brep(&mut self) {
        self.clean_geometry();
        self.generate_geometry();
    }

    pub fn clean_geometry(&mut self) {
        self.brep.clear();
    }

    #[wasm_bindgen]
    pub fn generate_geometry(&mut self) {
        let half_width = self.width / 2.0;
        let half_height = self.height / 2.0;
        let half_depth = self.depth / 2.0;

        let x_min = self.center.x - half_width;
        let x_max = self.center.x + half_width;
        let y_min = self.center.y - half_height;
        let y_max = self.center.y + half_height;
        let z_min = self.center.z - half_depth;
        let z_max = self.center.z + half_depth;

        self.brep
            .vertices
            .push(Vertex::new(0, Vector3::new(x_min, y_min, z_min)));
        self.brep
            .vertices
            .push(Vertex::new(1, Vector3::new(x_max, y_min, z_min)));
        self.brep
            .vertices
            .push(Vertex::new(2, Vector3::new(x_min, y_max, z_min)));
        self.brep
            .vertices
            .push(Vertex::new(3, Vector3::new(x_min, y_min, z_max)));
        self.brep
            .vertices
            .push(Vertex::new(4, Vector3::new(x_max, y_min, z_max)));
        self.brep
            .vertices
            .push(Vertex::new(5, Vector3::new(x_min, y_max, z_max)));

        self.brep.edges.push(Edge::new(0, 0, 1));
        self.brep.edges.push(Edge::new(1, 1, 4));
        self.brep.edges.push(Edge::new(2, 4, 3));
        self.brep.edges.push(Edge::new(3, 3, 0));
        self.brep.edges.push(Edge::new(4, 0, 2));
        self.brep.edges.push(Edge::new(5, 2, 5));
        self.brep.edges.push(Edge::new(6, 5, 3));
        self.brep.edges.push(Edge::new(7, 1, 2));
        self.brep.edges.push(Edge::new(8, 4, 5));

        self.brep.faces.push(Face::new(0, vec![0, 1, 4, 3]));
        self.brep.faces.push(Face::new(1, vec![0, 2, 1]));
        self.brep.faces.push(Face::new(2, vec![3, 4, 5]));
        self.brep.faces.push(Face::new(3, vec![0, 3, 5, 2]));
        self.brep.faces.push(Face::new(4, vec![1, 2, 5, 4]));
    }

    #[wasm_bindgen]
    pub fn get_brep_serialized(&self) -> String {
        serde_json::to_string(&self.brep).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_geometry_serialized(&self) -> String {
        let mut vertex_buffer: Vec<f64> = Vec::new();
        let faces = self.brep.faces.clone();

        for face in &faces {
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

        for edge in self.brep.edges.clone() {
            let start_vertex = self.brep.vertices[edge.v1 as usize].clone();
            let end_vertex = self.brep.vertices[edge.v2 as usize].clone();

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

impl OGWedge {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
    }
}
