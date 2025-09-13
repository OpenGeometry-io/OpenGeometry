use openmaths::Vector3;
use serde::{de, Deserialize, Serialize};
// Reference - https://15362.courses.cs.cmu.edu/spring2025content/lectures/12_rec3/12_rec3_slides.pdf
// Helpers can be added as needed

#[derive(Clone, Serialize, Deserialize)]
pub struct Vertex {
  pub id: u32,
  pub position: Vector3,
  
  // TODO: Add support for halfedges
  // pub edges: Vec<u32>,
  // pub halfedges: Vec<u32>,
}

impl Vertex {
  pub fn new(id: u32, position: Vector3) -> Self {
    Vertex {
      id,
      position,
      // edges: Vec::new(),
      // halfedges: Vec::new(),
    }
  }
}
