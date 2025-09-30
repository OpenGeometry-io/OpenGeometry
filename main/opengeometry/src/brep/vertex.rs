use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use crate::brep::halfedge::HalfEdge;
// Reference - https://15362.courses.cs.cmu.edu/spring2025content/lectures/12_rec3/12_rec3_slides.pdf
// Helpers can be added as needed

#[derive(Clone, Serialize, Deserialize)]
pub struct Vertex {
  pub id: u32,
  pub position: Vector3,
  
  // TODO: Add support for halfedges
  // Since Rust does not support pointers directly, either use Index or TODO: Add Direct Reference to HalfEdge struct
  pub halfedge_arbitary: i32, // Index of an arbitrary halfedge originating from this vertex - it can be -1 if no halfedge is associated
  // pub halfedge_struct: HalfEdge, // I Think this would be more complex to manage or maybe not
}

impl Vertex {
  pub fn new(id: u32, position: Vector3) -> Self {
    Vertex {
      id,
      position,
      halfedge_arbitary: -1,
      // halfedge_struct: HalfEdge { id: 0, twin_ref: 0, next_ref: 0, edge_ref: 0, vertex_ref: 0, face_ref: 0 },
    }
  }
}
