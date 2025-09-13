// Rename this file to buffergeometry.rs

use openmaths::Vector3;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct BufferGeometry {
  pub id: Uuid,
  pub vertices: Vec<Vector3>,
  pub indices: Vec<u32>,
}

impl BufferGeometry {
  pub fn new(id: Uuid) -> Self {
    BufferGeometry {
      id,
      vertices: Vec::new(),
      indices: Vec::new(),
    }
  }

  pub fn add_vertex(&mut self, vertex: Vector3) {
    self.vertices.push(vertex);
  }

  pub fn add_index(&mut self, index: u32) {
    self.indices.push(index);
  }

  pub fn clear(&mut self) {
    self.vertices.clear();
    self.indices.clear();
  }

  pub fn get_vertices(&self) -> &Vec<Vector3> {
    &self.vertices
  }

  pub fn get_indices(&self) -> &Vec<u32> {
    &self.indices
  }

  pub fn get_geometry_(&self) -> String {
    // serialize geometry
    let serialized = serde_json::to_string(&self).unwrap();
    serialized
  }
}

