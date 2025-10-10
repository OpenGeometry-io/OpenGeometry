use openmaths::Vector3;
use serde::{Serialize, Deserialize};

// Reference - https://15362.courses.cs.cmu.edu/spring2025content/lectures/12_rec3/12_rec3_slides.pdf
// Reference - https://15462.courses.cs.cmu.edu/spring2021content/lectures/11_meshes/11_meshes_slides.pdf
// Reference - https://doc.cgal.org/latest/HalfedgeDS/index.html
// Reference - https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html

/// Face in half-edge data structure
/// Following CGAL/OpenCASCADE pattern where each face references one boundary half-edge
#[derive(Clone, Serialize, Deserialize)]
pub struct Face {
  pub id: u32,
  pub normal: Vector3,
  
  /// Reference to one half-edge on the face boundary (CGAL pattern)
  /// This allows traversal of all half-edges around the face
  pub halfedge: Option<u32>,
  
  /// Holes in the face (each hole is represented by a half-edge)
  pub holes: Vec<u32>,  // Half-edge IDs for hole boundaries
  
  /// Legacy vertex indices (maintained for compatibility during transition)
  pub face_indices: Vec<u32>,
  
  /// Face properties
  pub is_valid: bool,
  pub area: f64,  // Cached area computation
}

impl Face {
  /// Create a new face with legacy vertex indices
  pub fn new(id: u32, face_indices: Vec<u32>) -> Self {
    Face {
      id,
      normal: Vector3::new(0.0, 0.0, 0.0), // Default normal, should be calculated later
      halfedge: None,
      holes: Vec::new(),
      face_indices,
      is_valid: true,
      area: 0.0,
    }
  }
  
  /// Create a face with half-edge boundary
  pub fn new_with_halfedge(id: u32, halfedge: u32) -> Self {
    Face {
      id,
      normal: Vector3::new(0.0, 0.0, 0.0),
      halfedge: Some(halfedge),
      holes: Vec::new(),
      face_indices: Vec::new(),  // Will be computed from half-edges
      is_valid: true,
      area: 0.0,
    }
  }
  
  /// Set the boundary half-edge
  pub fn set_halfedge(&mut self, halfedge_id: u32) {
    self.halfedge = Some(halfedge_id);
  }
  
  /// Get the boundary half-edge
  pub fn get_halfedge(&self) -> Option<u32> {
    self.halfedge
  }
  
  /// Add a hole to the face
  pub fn add_hole(&mut self, hole_halfedge: u32) {
    self.holes.push(hole_halfedge);
  }
  
  /// Get all holes
  pub fn get_holes(&self) -> &Vec<u32> {
    &self.holes
  }
  
  /// Check if face has holes
  pub fn has_holes(&self) -> bool {
    !self.holes.is_empty()
  }
  
  /// Set the face normal
  pub fn set_normal(&mut self, normal: Vector3) {
    self.normal = normal;
  }
  
  /// Get the face normal
  pub fn get_normal(&self) -> Vector3 {
    self.normal
  }
  
  /// Set cached area
  pub fn set_area(&mut self, area: f64) {
    self.area = area;
  }
  
  /// Get cached area
  pub fn get_area(&self) -> f64 {
    self.area
  }
  
  // Legacy methods for backward compatibility
  
  /// Get legacy face indices (for compatibility)
  pub fn get_face_indices(&self) -> &Vec<u32> {
    &self.face_indices
  }

  /// Get legacy indices count (for compatibility)
  pub fn get_indices_count(&self) -> u32 {
    self.face_indices.len() as u32
  }

  /// Insert vertex in legacy mode (for compatibility)
  pub fn insert_vertex(&mut self, vertex_id: u32) {
    self.face_indices.push(vertex_id);
  }
  
  /// Check if face has half-edge connectivity
  pub fn has_halfedge(&self) -> bool {
    self.halfedge.is_some()
  }
  
  /// Invalidate face (for error handling)
  pub fn invalidate(&mut self) {
    self.is_valid = false;
  }
}
