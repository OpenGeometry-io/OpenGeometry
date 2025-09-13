use openmaths::Vector3;
// use crate::brep_ds::halfedge::HalfEdge;
use serde::{Serialize, Deserialize};

// Reference - https://15362.courses.cs.cmu.edu/spring2025content/lectures/12_rec3/12_rec3_slides.pdf
// Reference - https://15462.courses.cs.cmu.edu/spring2021content/lectures/11_meshes/11_meshes_slides.pdf
// Helpers can be added as needed

#[derive(Clone, Serialize, Deserialize)]
pub struct Face {
  pub id: u32,
  pub normal: Vector3,
  pub face_indices: Vec<u32>,
  // TODO: Add support for halfedges
  // pub halfedge: HalfEdge,
}

impl Face {
  pub fn new(id: u32, face_indices: Vec<u32>) -> Self {
    Face {
      id,
      normal: Vector3::new(0.0, 0.0, 0.0), // Default normal, should be calculated later
      face_indices,
      // halfedge: HalfEdge::new(0, 0, 0), // Placeholder for halfedge
    }
  }

  pub fn set_normal(&mut self, normal: Vector3) {
    self.normal = normal;
  }

  pub fn get_face_indices(&self) -> &Vec<u32> {
    &self.face_indices
  }

  pub fn get_indices_count(&self) -> u32 {
    self.face_indices.len() as u32
  }

  pub fn insert_vertex(&mut self, vertex_id: u32) {
    self.face_indices.push(vertex_id);
  }
}
