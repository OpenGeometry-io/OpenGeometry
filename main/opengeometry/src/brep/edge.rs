// use crate::brep::halfedge::HalfEdge;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Edge {
  pub id: u32,
  pub v1: u32,
  pub v2: u32,
  // TODO: Add support for halfedges
  // pub halfedges: Vec<HalfEdge>,
  // halfedge: HalfEdge,
}

impl Edge {
  pub fn new(id: u32, v1: u32, v2: u32) -> Self {
    Edge {
      id,
      v1,
      v2
    }
  }
}
