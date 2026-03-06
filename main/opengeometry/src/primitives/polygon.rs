use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::brep::{Brep, BrepBuilder};
use crate::export::projection::{
    collect_visible_outline_vertex_buffer, project_brep_to_scene, CameraParameters, HlrOptions,
    ProjectionMode, Scene2D,
};
use crate::operations::triangulate::triangulate_polygon_with_holes;
use crate::utility::bgeometry::BufferGeometry;
use openmaths::{Matrix4, Vector3};
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGPolygon {
    id: String,
    points: Vec<Vector3>,
    holes: Vec<Vec<Vector3>>,
    geometry: BufferGeometry,
    brep: Brep,
}

impl OGPolygon {
    pub fn brep(&self) -> &Brep {
        &self.brep
    }

    pub fn to_projected_scene2d(&self, camera: &CameraParameters, hlr: &HlrOptions) -> Scene2D {
        project_brep_to_scene(&self.brep, camera, hlr)
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
            geometry: BufferGeometry::new(internal_id),
            brep: Brep::new(internal_id),
        }
    }

    #[wasm_bindgen]
    pub fn set_config(&mut self, points: Vec<Vector3>) -> Result<(), JsValue> {
        self.points = points;
        self.holes.clear();
        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn set_transformation(&mut self, transformation: Vec<f64>) -> Result<(), JsValue> {
        if transformation.len() != 16 {
            return Err(JsValue::from_str(
                "Transformation matrix must have exactly 16 elements",
            ));
        }

        let transformation_matrix: Matrix4 = Matrix4::set(
            transformation[0],
            transformation[4],
            transformation[8],
            transformation[12],
            transformation[1],
            transformation[5],
            transformation[9],
            transformation[13],
            transformation[2],
            transformation[6],
            transformation[10],
            transformation[14],
            transformation[3],
            transformation[7],
            transformation[11],
            transformation[15],
        );

        for point in &mut self.points {
            point.apply_matrix4(transformation_matrix.clone());
        }

        for hole in &mut self.holes {
            for point in hole {
                point.apply_matrix4(transformation_matrix.clone());
            }
        }

        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn add_vertices(&mut self, vertices: Vec<Vector3>) -> Result<(), JsValue> {
        self.points = vertices;
        self.holes.clear();
        self.generate_brep()
    }

    #[wasm_bindgen]
    pub fn add_holes(&mut self, hole: Vec<Vector3>) -> Result<(), JsValue> {
        self.holes.push(hole);
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

        let mut all_vertices = self.points.clone();
        let mut hole_index_sets: Vec<Vec<u32>> = Vec::new();

        for hole in &self.holes {
            if hole.len() < 3 {
                continue;
            }

            let start = all_vertices.len() as u32;
            all_vertices.extend(hole.iter().copied());
            let indices: Vec<u32> = (0..hole.len() as u32)
                .map(|offset| start + offset)
                .collect();
            hole_index_sets.push(indices);
        }

        builder.add_vertices(&all_vertices);
        let outer_indices: Vec<u32> = (0..self.points.len() as u32).collect();

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
