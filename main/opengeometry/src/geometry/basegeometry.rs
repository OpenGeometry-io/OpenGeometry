/*
 * Base Geometry Module
 * 
 * The Base Geomtry is a base trait that all geometry types should implement.
 * 
 * Instead of Serde bindgen can be directly used for getting values from struct - https://github.com/rustwasm/wasm-bindgen/issues/439
 */

use openmaths::Vector3;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct BaseGeometry {
  id: String,
  vertices: Vec<Vector3>,
  indices: Vec<u32>,
  normals: Vec<f32>,
  treated: bool,
  buffer: Vec<Vector3>,
  holes: Vec<Vec<Vector3>>,
  flat_vertices: Vec<f64>,
  pub ccw: bool,
}

impl BaseGeometry {
  // Why Getter and Setter - https://github.com/rustwasm/wasm-bindgen/issues/1775
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  pub fn id(&self) -> String {
    self.id.clone()
  }

  pub fn new(id: String) -> BaseGeometry {
    BaseGeometry {
      id,
      vertices: Vec::new(),
      indices: Vec::new(),
      normals: Vec::new(),
      treated: false,
      buffer: Vec::new(),
      holes: Vec::new(),
      flat_vertices: Vec::new(),
      ccw: false,
    }
  }

  pub fn set_ccw(&mut self, ccw: bool) {
    self.ccw = ccw;
  }
  
  pub fn add_vertices(&mut self, vertices: Vec<Vector3>) {
    for vertex in vertices {
      self.vertices.push(vertex.clone());

      self.flat_vertices.push(vertex.x);
      self.flat_vertices.push(vertex.y);
      self.flat_vertices.push(vertex.z);

      self.indices.push(self.vertices.len() as u32 - 1);
    }
  }

  pub fn add_indices(&mut self, indices: Vec<u32>) {
    for index in indices {
      self.indices.push(index);
    }
  }

  pub fn get_indices(&self) -> Vec<u32> {
    self.indices.clone()
  }
  
  pub fn add_holes(&mut self, holes: Vec<Vector3>) {
    self.holes.push(holes.clone());
  }

  
  pub fn get_holes(&mut self) -> Vec<Vec<Vector3>> {
    self.holes.clone()
  }

  
  pub fn add_vertex(&mut self, vertex: Vector3) {
    self.vertices.push(vertex.clone());
    self.indices.push(self.vertices.len() as u32 - 1);
  }

  pub fn add_index(&mut self, index: u32) {
    self.indices.push(index);
  }
  
  pub fn add_normal(&mut self, normal: f32) {
    self.normals.push(normal);
  }
  
  pub fn clone_geometry(&self) -> BaseGeometry {
    self.clone()
  }
  
  pub fn get_vertices(&self) -> Vec<Vector3> {
    self.vertices.clone()
  }
    
  pub fn get_geometry(&self) -> String {
    let geometry = serde_json::to_string(&self).unwrap();
    geometry
  }

  // pub fn get_dimension_for_vertex (&self) -> u32 {
  //   self.vertices[0].
  // }

  pub fn translate(&mut self, translation: Vector3) {
    for vertex in &mut self.vertices {
      vertex.x += translation.x;
      vertex.y += translation.y;
      vertex.z += translation.z;
    }
  }
  
  pub fn reset_geometry(&mut self) {
    self.vertices.clear();
    self.flat_vertices.clear();
    self.indices.clear();
    self.normals.clear();
    self.holes.clear();
    self.treated = false;
    self.buffer.clear();
  }
}
