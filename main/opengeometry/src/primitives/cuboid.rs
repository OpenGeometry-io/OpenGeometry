/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Box primitive for OpenGeometry.
 * 
 * Base created by default on XZ plane and extruded along Y axis.
 * 
 * There are two ways to create a box:
 * 1. By creating a box with a rectangle face, create a Rectangle Poly Face and then extrude by a given height
 * 2. By creating a box primitive with given width, height, and depth
 * 
 * This class is used to create a box primitive(2) using width, height, and depth.
 */
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use crate::brep::{Edge, Face, Brep, Vertex};
use crate::operations::extrude::extrude_brep_face;
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
      id: id.clone(),
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

    let mut bottom_face_brep = Brep::new(Uuid::new_v4());
    bottom_face_brep.vertices.push(Vertex::new(0, Vector3::new(self.center.x - half_width, self.center.y - half_height, self.center.z - half_depth)));
    bottom_face_brep.vertices.push(Vertex::new(1, Vector3::new(self.center.x + half_width, self.center.y - half_height, self.center.z - half_depth)));
    bottom_face_brep.vertices.push(Vertex::new(2, Vector3::new(self.center.x + half_width, self.center.y - half_height, self.center.z + half_depth)));
    bottom_face_brep.vertices.push(Vertex::new(3, Vector3::new(self.center.x - half_width, self.center.y - half_height, self.center.z + half_depth)));

    bottom_face_brep.edges.push(Edge::new(0, 0, 1));
    bottom_face_brep.edges.push(Edge::new(1, 1, 2));
    bottom_face_brep.edges.push(Edge::new(2, 2, 3));
    bottom_face_brep.edges.push(Edge::new(3, 3, 0));

    bottom_face_brep.faces.push(Face::new(0, vec![0, 1, 2, 3]));

    // Extrude the bottom face to create the box
    let extruded_brep = extrude_brep_face(bottom_face_brep, self.height);
    self.brep = extruded_brep.clone();
  }

  #[wasm_bindgen]
  pub fn get_brep_serialized(&self) -> String {
    let serialized = serde_json::to_string(&self.brep).unwrap();
    serialized
  }

  #[wasm_bindgen]
  pub fn get_geometry_serialized(&self) -> String {
    let mut vertex_buffer: Vec<f64> = Vec::new();
    let faces = self.brep.faces.clone();

    for face in &faces {
      let (face_vertices, holes_vertices) = self.brep.get_vertices_and_holes_by_face_id(face.id);

      if face_vertices.len() < 3 {
        continue;
      }

      let triangles = triangulate_polygon_with_holes(&face_vertices, &holes_vertices);

      // Combine outer and hole vertices into a single list for easy lookup
      let all_vertices: Vec<Vector3> = face_vertices
        .into_iter()
        .chain(holes_vertices.into_iter().flatten())
        .collect();

      // Build the final vertex buffer for rendering
      for triangle in triangles {
        for vertex_index in triangle {
          // The indices from earcutr correspond to our combined `all_vertices` list
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
  pub fn get_outline_geometry_serialized(&mut self) -> String {
    let mut vertex_buffer: Vec<f64> = Vec::new();

    let edges = self.brep.edges.clone();
    for edge in edges {
      let start_index = edge.v1 as usize;
      let end_index = edge.v2 as usize;

      let start_vertex = self.brep.vertices[start_index].clone();
      let end_vertex = self.brep.vertices[end_index].clone();

      vertex_buffer.push(start_vertex.position.x);
      vertex_buffer.push(start_vertex.position.y);
      vertex_buffer.push(start_vertex.position.z);

      vertex_buffer.push(end_vertex.position.x);
      vertex_buffer.push(end_vertex.position.y);
      vertex_buffer.push(end_vertex.position.z);
    }

    if self.brep.hole_edges.len() > 0 {
      for edge in self.brep.hole_edges.clone() {
        let start_index = edge.v1 as usize;
        let end_index = edge.v2 as usize;

        let start_vertex = self.brep.vertices[start_index].clone();
        let end_vertex = self.brep.vertices[end_index].clone();

        vertex_buffer.push(start_vertex.position.x);
        vertex_buffer.push(start_vertex.position.y);
        vertex_buffer.push(start_vertex.position.z);

        vertex_buffer.push(end_vertex.position.x);
        vertex_buffer.push(end_vertex.position.y);
        vertex_buffer.push(end_vertex.position.z);
      }
    }

    let vertex_serialized = serde_json::to_string(&vertex_buffer).unwrap();
    vertex_serialized
  }
}