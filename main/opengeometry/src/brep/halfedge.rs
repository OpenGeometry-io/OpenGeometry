// Reference - https://15362.courses.cs.cmu.edu/spring2025content/lectures/12_rec3/12_rec3_slides.pdf
// Reference - https://doc.cgal.org/latest/HalfedgeDS/index.html
// Reference - https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html#occt_modat_5_1

use serde::{Serialize, Deserialize};

/// Half-edge data structure based on CGAL and OpenCASCADE patterns
/// Each half-edge represents a directed edge with complete topological connectivity
#[derive(Clone, Serialize, Deserialize)]
pub struct HalfEdge {
  pub id: u32,

  // Core half-edge topology (CGAL/OpenCASCADE pattern)
  pub twin: Option<u32>,     // Twin half-edge (opposite direction)
  pub next: Option<u32>,     // Next half-edge in face boundary
  pub prev: Option<u32>,     // Previous half-edge in face boundary
  
  // Geometric references
  pub vertex: u32,           // Target vertex (vertex this half-edge points to)
  pub edge: Option<u32>,     // Parent edge (shared with twin)
  pub face: Option<u32>,     // Incident face (None for boundary half-edges)
  
  // Flags for robust handling
  pub is_boundary: bool,     // True if this is a boundary half-edge (no face)
  pub is_valid: bool,        // Validity flag for error handling
}

impl HalfEdge {
  /// Create a new half-edge with minimal connectivity
  pub fn new(id: u32, vertex: u32) -> Self {
    HalfEdge {
      id,
      twin: None,
      next: None,
      prev: None,
      vertex,
      edge: None,
      face: None,
      is_boundary: true,  // Default to boundary until assigned to face
      is_valid: true,
    }
  }
  
  /// Create a complete half-edge with all connectivity
  pub fn new_complete(
    id: u32,
    vertex: u32,
    twin: Option<u32>,
    next: Option<u32>,
    prev: Option<u32>,
    edge: Option<u32>,
    face: Option<u32>
  ) -> Self {
    HalfEdge {
      id,
      twin,
      next,
      prev,
      vertex,
      edge,
      face,
      is_boundary: face.is_none(),
      is_valid: true,
    }
  }
  
  /// Set twin half-edge (bidirectional linking)
  pub fn set_twin(&mut self, twin_id: u32) {
    self.twin = Some(twin_id);
  }
  
  /// Set next half-edge in face loop
  pub fn set_next(&mut self, next_id: u32) {
    self.next = Some(next_id);
  }
  
  /// Set previous half-edge in face loop
  pub fn set_prev(&mut self, prev_id: u32) {
    self.prev = Some(prev_id);
  }
  
  /// Set parent edge
  pub fn set_edge(&mut self, edge_id: u32) {
    self.edge = Some(edge_id);
  }
  
  /// Set incident face
  pub fn set_face(&mut self, face_id: u32) {
    self.face = Some(face_id);
    self.is_boundary = false;
  }
  
  /// Mark as boundary half-edge (no face)
  pub fn set_boundary(&mut self) {
    self.face = None;
    self.is_boundary = true;
  }
  
  /// Check if half-edge has complete connectivity
  pub fn is_complete(&self) -> bool {
    self.twin.is_some() && 
    self.next.is_some() && 
    self.prev.is_some() && 
    self.edge.is_some()
  }
  
  /// Invalidate half-edge (for error handling)
  pub fn invalidate(&mut self) {
    self.is_valid = false;
  }
  
  /// Get the source vertex (from twin's target)
  pub fn get_source_vertex(&self) -> Option<u32> {
    // Source vertex is the target vertex of the twin
    // This requires looking up the twin in the parent structure
    self.twin
  }
  
  /// Get target vertex
  pub fn get_target_vertex(&self) -> u32 {
    self.vertex
  }
}
