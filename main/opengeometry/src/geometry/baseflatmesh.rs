/**
 * As of now we can consider this as a 2D Mesh
 * It's y value is always 0
 * Potientally, it will be used for Creation of Walls and Other 2D Meshes
 */

use crate::operations::triangulate::triangulate_polygon_buffer_geometry;
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use super::basegeometry;
use openmaths::Vector3;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct BaseFlatMesh {
  id: String,
  geometry: basegeometry::BaseGeometry,
  pub extruded: bool,
  pub is_mesh: bool,
  pub position: Vector3,
  pub rotation: Vector3,
  pub scale: Vector3,
  buffer: Vec<f64>
}

#[wasm_bindgen]
impl BaseFlatMesh {
  // Why Getter and Setter - https://github.com/rustwasm/wasm-bindgen/issues/1775
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> BaseFlatMesh {
    let geometry_id = id.clone();
    BaseFlatMesh {
      id,
      geometry : basegeometry::BaseGeometry::new(geometry_id.clone()),
      extruded : false,
      is_mesh : false,
      position : Vector3::new(0.0, 0.0, 0.0),
      rotation : Vector3::new(0.0, 0.0, 0.0),
      scale : Vector3::new(1.0, 1.0, 1.0),
      buffer : Vec::new()
    }
  }

  #[wasm_bindgen]
  pub fn add_vertices(&mut self, vertices: Vec<Vector3>) {
    self.geometry.add_vertices(vertices);
  }

  #[wasm_bindgen]
  pub fn add_vertex(&mut self, vertex: Vector3) {
    self.geometry.add_vertex(vertex);
    
    if self.geometry.get_vertices().len() > 2 {
      self.is_mesh = true;
    }
  }

  #[wasm_bindgen]
  pub fn triangulate(&mut self) -> String {
    self.is_mesh = true;

    let merged_vertices = self.geometry.get_vertices();
    let indices = triangulate_polygon_buffer_geometry(self.geometry.clone());

    // let ccw_vertices = windingsort::ccw_test(self.geometry.get_vertices());
    
    // for index in indices {
    //   for i in index {
    //     let vertex = ccw_vertices[i as usize];
    //     self.buffer.push(vertex.x);
    //     self.buffer.push(vertex.y);
    //     self.buffer.push(vertex.z);
    //   }
    // }

    // serde_json::to_string(&self.buffer).unwrap()

    serde_json::to_string(&merged_vertices).unwrap()
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
