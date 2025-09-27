use core::str;
#[cfg(feature="wasm")] use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use crate::brep::{Edge, Face, Brep, Vertex};
use crate::operations::extrude::extrude_brep_face;
use crate::operations::triangulate::triangulate_polygon_by_face;
// use crate::utility::bgeometry::BufferGeometry;
use openmaths::Vector3;
use uuid::Uuid;

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

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGCube {
  id: String,
  center: Vector3,
  width: f64,
  height: f64,
  depth: f64,
  brep: Brep,
}


impl OGCube {
  
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  
  pub fn id(&self) -> String {
    self.id.clone()
  }

  
  pub fn new(id: String) -> OGCube {

    let internal_id = Uuid::new_v4();

    OGCube {
      id: id.clone(),
      center: Vector3::new(0.0, 0.0, 0.0),
      width: 1.0,
      height: 1.0,
      depth: 1.0,
      brep: Brep::new(internal_id),
    }
  }

  pub fn clean_geometry(&mut self) {
    self.brep.clear();
  }

  
  pub fn set_config(&mut self, center: Vector3, width: f64, height: f64, depth: f64) {
    self.center = center;
    self.width = width;
    self.height = height;
    self.depth = depth;
  }

  
  pub fn generate_geometry(&mut self) {
    let half_width = self.width / 2.0;
    let half_height = self.height / 2.0;
    let half_depth = self.depth / 2.0;

    // Clear the existing BREP data
    self.clean_geometry();

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

  
  pub fn get_brep_dump(&self) -> String {
    serde_json::to_string(&self.brep).unwrap()
  }

  
  pub fn get_geometry_serialized(&self) -> String {
    let mut vertex_buffer: Vec<f64> = Vec::new();
    let faces = self.brep.faces.clone();

    for i in 0..faces.len() {
      let face = faces[i].clone();
      let face_vertices = self.brep.get_vertices_by_face_id(face.id);
      // Triangulate the face vertices
      let triangulated_face_indices = triangulate_polygon_by_face(face_vertices.clone());
      for index in triangulated_face_indices {
        for vertex_id in index {
          let vertex = face_vertices[vertex_id as usize].clone();
          vertex_buffer.push(vertex.x);
          vertex_buffer.push(vertex.y);
          vertex_buffer.push(vertex.z);
        }
      }
    }

    let vertex_serialized = serde_json::to_string(&vertex_buffer).unwrap();
    vertex_serialized
  }

    
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

    let vertex_serialized = serde_json::to_string(&vertex_buffer).unwrap();
    vertex_serialized
  }
}

// WASM-specific implementation
#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl OGCube {
  #[wasm_bindgen(constructor)]
  pub fn new_wasm(id: String) -> OGCube {
    OGCube::new(id)
  }

  #[wasm_bindgen(setter)]
  pub fn set_id_wasm(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id_wasm(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen]
  pub fn set_config_wasm(&mut self, center: Vector3, width: f64, height: f64, depth: f64) {
    self.set_config(center, width, height, depth);
  }

  #[wasm_bindgen]
  pub fn generate_geometry_wasm(&mut self) {
    self.generate_geometry();
  }

  #[wasm_bindgen]
  pub fn get_geometry_serialized_wasm(&self) -> String {
    self.get_geometry_serialized()
  }
}