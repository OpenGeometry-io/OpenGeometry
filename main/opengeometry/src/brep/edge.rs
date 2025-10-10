use serde::{Serialize, Deserialize};
// Reference - https://doc.cgal.org/latest/HalfedgeDS/index.html
// Reference - https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html

/// Edge in half-edge data structure
/// Following CGAL/OpenCASCADE pattern where each edge references its two half-edges
#[derive(Clone, Serialize, Deserialize)]
pub struct Edge {
  pub id: u32,
  
  /// The two half-edges that bound this edge
  /// halfedge1 goes from v1 to v2, halfedge2 goes from v2 to v1
  pub halfedge1: Option<u32>,  // Half-edge in one direction
  pub halfedge2: Option<u32>,  // Half-edge in opposite direction (twin of halfedge1)
  
  /// Vertex endpoints (maintained for compatibility and efficiency)
  pub v1: u32,  // Source vertex
  pub v2: u32,  // Target vertex
  
  /// Edge properties
  pub is_boundary: bool,  // True if this edge is on the boundary (one face)
  pub is_valid: bool,     // Validity flag for error handling
}

impl Edge {
  /// Create a new edge with vertex endpoints
  pub fn new(id: u32, v1: u32, v2: u32) -> Self {
    Edge {
      id,
      halfedge1: None,
      halfedge2: None,
      v1,
      v2,
      is_boundary: true,  // Default to boundary until both half-edges assigned
      is_valid: true,
    }
  }
  
  /// Create edge with half-edge references
  pub fn new_with_halfedges(id: u32, v1: u32, v2: u32, he1: u32, he2: u32) -> Self {
    Edge {
      id,
      halfedge1: Some(he1),
      halfedge2: Some(he2),
      v1,
      v2,
      is_boundary: false,
      is_valid: true,
    }
  }
  
  /// Set the first half-edge (v1 -> v2)
  pub fn set_halfedge1(&mut self, halfedge_id: u32) {
    self.halfedge1 = Some(halfedge_id);
    self.update_boundary_status();
  }
  
  /// Set the second half-edge (v2 -> v1)
  pub fn set_halfedge2(&mut self, halfedge_id: u32) {
    self.halfedge2 = Some(halfedge_id);
    self.update_boundary_status();
  }
  
  /// Get the half-edge going from v1 to v2
  pub fn get_halfedge1(&self) -> Option<u32> {
    self.halfedge1
  }
  
  /// Get the half-edge going from v2 to v1
  pub fn get_halfedge2(&self) -> Option<u32> {
    self.halfedge2
  }
  
  /// Check if edge has both half-edges
  pub fn has_both_halfedges(&self) -> bool {
    self.halfedge1.is_some() && self.halfedge2.is_some()
  }
  
  /// Update boundary status based on half-edge presence
  fn update_boundary_status(&mut self) {
    self.is_boundary = !(self.halfedge1.is_some() && self.halfedge2.is_some());
  }
  
  /// Get the other vertex given one vertex
  pub fn get_other_vertex(&self, vertex: u32) -> Option<u32> {
    if vertex == self.v1 {
      Some(self.v2)
    } else if vertex == self.v2 {
      Some(self.v1)
    } else {
      None
    }
  }
  
  /// Check if vertex is incident to this edge
  pub fn contains_vertex(&self, vertex: u32) -> bool {
    vertex == self.v1 || vertex == self.v2
  }
  
  /// Invalidate edge (for error handling)
  pub fn invalidate(&mut self) {
    self.is_valid = false;
  }
}
