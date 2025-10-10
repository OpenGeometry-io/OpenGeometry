use openmaths::Vector3;
use serde::{de, Deserialize, Serialize};
// Reference - https://15362.courses.cs.cmu.edu/spring2025content/lectures/12_rec3/12_rec3_slides.pdf
// Reference - https://doc.cgal.org/latest/HalfedgeDS/index.html
// Reference - https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html

/// Vertex in half-edge data structure
/// Following CGAL/OpenCASCADE pattern where each vertex references one outgoing half-edge
#[derive(Clone, Serialize, Deserialize)]
pub struct Vertex {
  pub id: u32,
  pub position: Vector3,
  
  /// Reference to one outgoing half-edge (CGAL pattern)
  /// This allows traversal of all half-edges around the vertex
  pub halfedge: Option<u32>,
  
  /// Validity flag for error handling
  pub is_valid: bool,
}

impl Vertex {
  pub fn new(id: u32, position: Vector3) -> Self {
    Vertex {
      id,
      position,
      halfedge: None,
      is_valid: true,
    }
  }
  
  /// Create vertex with halfedge reference
  pub fn new_with_halfedge(id: u32, position: Vector3, halfedge: u32) -> Self {
    Vertex {
      id,
      position,
      halfedge: Some(halfedge),
      is_valid: true,
    }
  }
  
  /// Set the outgoing half-edge reference
  pub fn set_halfedge(&mut self, halfedge_id: u32) {
    self.halfedge = Some(halfedge_id);
  }
  
  /// Get the outgoing half-edge reference
  pub fn get_halfedge(&self) -> Option<u32> {
    self.halfedge
  }
  
  /// Check if vertex has half-edge connectivity
  pub fn has_halfedge(&self) -> bool {
    self.halfedge.is_some()
  }
  
  /// Invalidate vertex (for error handling)
  pub fn invalidate(&mut self) {
    self.is_valid = false;
  }
  
  /// Check if vertex is isolated (no half-edges)
  pub fn is_isolated(&self) -> bool {
    self.halfedge.is_none()
  }
}
