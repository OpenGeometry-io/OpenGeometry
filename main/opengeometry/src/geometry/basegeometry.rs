/*
 * Base Geometry Module
 * 
 * The Base Geomtry is a base trait that all geometry types should implement.
 * 
 * Instead of Serde bindgen can be directly used for getting values from struct - https://github.com/rustwasm/wasm-bindgen/issues/439
 */

use crate::utility::openmath;
 
use std::path;

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

// #[wasm_bindgen]
// #[derive(Clone, Serialize, Deserialize)]
// pub struct Buffer {
//   vertices: Vec<openmath::Vector3D>,
//   indices: Vec<u32>
// }

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct BaseGeometry {
  pub id: u32,
  vertices: Vec<openmath::Vector3D>,
  indices: Vec<u32>,
  normals: Vec<f32>,
  treated: bool,
  buffer: Vec<openmath::Vector3D>
}

#[wasm_bindgen]
impl BaseGeometry {
  #[wasm_bindgen(constructor)]
  pub fn new(id: u32) -> BaseGeometry {
    BaseGeometry {
      id,
      vertices: Vec::new(),
      indices: Vec::new(),
      normals: Vec::new(),
      treated: false,
      buffer: Vec::new()
    }
  }

  #[wasm_bindgen]
  pub fn add_vertices(&mut self, vertices: Vec<openmath::Vector3D>) {
    for vertex in vertices {
      self.vertices.push(vertex.clone());
    }
  }

  #[wasm_bindgen]
  pub fn add_vertex(&mut self, vertex: openmath::Vector3D) {
    self.vertices.push(vertex.clone());
  }

  #[wasm_bindgen]
  pub fn add_index(&mut self, index: u32) {
    self.indices.push(index);
  }

  #[wasm_bindgen]
  pub fn add_normal(&mut self, normal: f32) {
    self.normals.push(normal);
  }

  #[wasm_bindgen]
  pub fn clone_geometry(&self) -> BaseGeometry {
    self.clone()  // Use Clone to duplicate the struct safely.
  }

  #[wasm_bindgen]
  pub fn get_vertices(&self) -> Vec<openmath::Vector3D> {
    self.vertices.clone()
  }
  
  #[wasm_bindgen]
  pub fn get_geometry(&self) -> String {
    serde_json::to_string(&self).unwrap()
  }

  #[wasm_bindgen]
  pub fn reset_geometry(&mut self) {
    self.vertices.clear();
    self.indices.clear();
    self.normals.clear();
    self.treated = false;
    self.buffer.clear();
  }
}
