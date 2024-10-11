/**
 * As of now we can consider this as a 2D Mesh
 * It's y value is always 0
 * Potientally, it will be used for Creation of Walls and Other 2D Meshes
 */

use crate::{operations::{triangulate::triangulate_polygon_buffer_geometry, windingsort}, utility::openmath};
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use super::basegeometry;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct BaseFlatMesh {
  pub id: u32,
  geometry: basegeometry::BaseGeometry,
  pub extruded: bool,
  pub is_mesh: bool,
  pub position: openmath::Vector3D,
  pub rotation: openmath::Vector3D,
  pub scale: openmath::Vector3D,
  buffer: Vec<f64>
}

#[wasm_bindgen]
impl BaseFlatMesh {
  #[wasm_bindgen(constructor)]
  pub fn new(id: u32) -> BaseFlatMesh {
    BaseFlatMesh {
      id,
      geometry : basegeometry::BaseGeometry::new(id),
      extruded : false,
      is_mesh : false,
      position : openmath::Vector3D::create(0.0, 0.0, 0.0),
      rotation : openmath::Vector3D::create(0.0, 0.0, 0.0),
      scale : openmath::Vector3D::create(1.0, 1.0, 1.0),
      buffer : Vec::new()
    }
  }

  #[wasm_bindgen]
  pub fn add_vertices(&mut self, vertices: Vec<openmath::Vector3D>) {
    self.geometry.add_vertices(vertices);
  }

  #[wasm_bindgen]
  pub fn add_vertex(&mut self, vertex: openmath::Vector3D) {
    self.geometry.add_vertex(vertex);
    
    if self.geometry.get_vertices().len() > 2 {
      self.is_mesh = true;
    }
  }

  // #[wasm_bindgen]
  // pub fn triangulate(&mut self) -> String {
  //   if self.is_polygon {
  //     // Polygon is already triangulated, destroy the previous triangulation
  //     return String::from("Polygon is already triangulated");
  //   }

  //   if self.geometry.get_vertices().len() < 3 {
  //     return String::from("Polygon should have atleast 3 vertices");
  //   }

  //   self.is_polygon = true;
  //   triangulate_polygon_buffer_geometry(self.geometry.clone())
  // }

  #[wasm_bindgen]
  pub fn triangulate(&mut self) -> String {
    // if self.is_polygon {
    //   // Polygon is already triangulated, destroy the previous triangulation
    //   // return String::from("Polygon is already triangulated");
    // }

    // if self.geometry.get_vertices().len() < 3 {
    //   // return String::from("Polygon should have atleast 3 vertices");
    // }

    self.is_mesh = true;
    let indices = triangulate_polygon_buffer_geometry(self.geometry.clone());

    let ccw_vertices = windingsort::ccw_test(self.geometry.get_vertices());
    
    for index in indices {
      for i in index {
        let vertex = ccw_vertices[i as usize];
        self.buffer.push(vertex.x);
        self.buffer.push(vertex.y);
        self.buffer.push(vertex.z);
      }
    }

    serde_json::to_string(&self.buffer).unwrap()
  }

  #[wasm_bindgen]
  pub fn get_buffer_flush(&self) -> String {
    serde_json::to_string(&self.buffer).unwrap()
  }

  #[wasm_bindgen]
  pub fn reset_mesh(&mut self) {
    self.is_mesh = false;
    self.geometry.reset_geometry();
    self.buffer.clear();
  }
 
}
