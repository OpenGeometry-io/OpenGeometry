// Reference - https://15362.courses.cs.cmu.edu/spring2025content/lectures/12_rec3/12_rec3_slides.pdf
// I wish Rust had pointers

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct HalfEdge {
  pub id: u32,

  pub vertex_ref: u32, // ID of the vertex this halfedge is starting from

  pub twin_ref: u32, // ID of the twin halfedge

  pub next_ref: u32, // ID of the next halfedge in the loop
  
  pub edge_ref: u32, // ID of edge this halfedge belongs to
  
  pub face_ref: u32, // ID of the face this halfedge belongs to
  // The face would be on the left side of the halfedge when traversed from vertex_ref to the twin's vertex_ref
}
