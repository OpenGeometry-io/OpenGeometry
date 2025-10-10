/**
 * BRep Module/Structure
 * References - https://en.wikipedia.org/wiki/Boundary_representation
 * References - https://en.wikipedia.org/wiki/Doubly_connected_edge_list
 * References - https://en.wikipedia.org/wiki/Polygon_mesh
 * References - https://www.cs.cmu.edu/~./quake/robust.html
 * References - https://doc.cgal.org/latest/HalfedgeDS/index.html
 * References - https://dev.opencascade.org/doc/overview/html/occt_user_guides__modeling_data.html
 */
pub mod vertex;
pub mod edge;
pub mod halfedge;
pub mod face;

use openmaths::Vector3;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::collections::HashMap;

// Import and re-export types
pub use vertex::Vertex;
pub use edge::Edge;
pub use halfedge::HalfEdge;
pub use face::Face;

#[derive(Clone, Serialize, Deserialize)]
pub struct Brep {
  pub id: Uuid,
  pub vertices: Vec<Vertex>,
  pub edges: Vec<Edge>,
  pub halfedges: Vec<HalfEdge>,
  pub faces: Vec<Face>,
  
  // Legacy hole support (will be replaced by half-edge holes)
  pub holes: Vec<u32>,
  pub hole_edges: Vec<Edge>,
  
  // Counters for generating unique IDs
  next_vertex_id: u32,
  next_edge_id: u32,
  next_halfedge_id: u32,
  next_face_id: u32,
}

impl Brep {
  pub fn new(id: Uuid) -> Self {
    Brep {
      id,
      vertices: Vec::new(),
      edges: Vec::new(),
      halfedges: Vec::new(),
      faces: Vec::new(),
      holes: Vec::new(),
      hole_edges: Vec::new(),
      next_vertex_id: 0,
      next_edge_id: 0,
      next_halfedge_id: 0,
      next_face_id: 0,
    }
  }

  pub fn clear(&mut self) {
    self.vertices.clear();
    self.edges.clear();
    self.halfedges.clear();
    self.faces.clear();
    self.holes.clear();
    self.hole_edges.clear();
    self.next_vertex_id = 0;
    self.next_edge_id = 0;
    self.next_halfedge_id = 0;
    self.next_face_id = 0;
  }

  pub fn get_vertex_count(&self) -> u32 {
    self.vertices.len() as u32
  }

  pub fn get_edge_count(&self) -> u32 {
    self.edges.len() as u32
  }

  pub fn get_halfedge_count(&self) -> u32 {
    self.halfedges.len() as u32
  }

  pub fn get_face_count(&self) -> u32 {
    self.faces.len() as u32
  }

  pub fn get_hole_edge_count(&self) -> u32 {
    self.hole_edges.len() as u32
  }

  // ID generation methods
  pub fn next_vertex_id(&mut self) -> u32 {
    let id = self.next_vertex_id;
    self.next_vertex_id += 1;
    id
  }

  pub fn next_edge_id(&mut self) -> u32 {
    let id = self.next_edge_id;
    self.next_edge_id += 1;
    id
  }

  pub fn next_halfedge_id(&mut self) -> u32 {
    let id = self.next_halfedge_id;
    self.next_halfedge_id += 1;
    id
  }

  pub fn next_face_id(&mut self) -> u32 {
    let id = self.next_face_id;
    self.next_face_id += 1;
    id
  }

  /**
   * Get vertices by face ID
   * @returns Vec<Vector3> - A vector of Vector3 representing the vertices of the face
   * Use this when we need vertices not just indices
   */
  pub fn get_vertices_by_face_id(&self, face_id: u32) -> Vec<Vector3> {
    let face = self.faces[face_id as usize].clone();
    let mut vertices = Vec::new();
    let face_index_count = face.get_indices_count();
    for index in 0..face_index_count {
      let vertex_id = face.face_indices[index as usize];
      let vertex = self.vertices[vertex_id as usize].clone();
      vertices.push(vertex.position);
    }
    vertices
  }

  pub fn insert_vertex_at_face_by_id(&mut self, face_id: u32, vertex_id: u32) {
    if let Some(face) = self.faces.iter_mut().find(|f| f.id == face_id) {
      face.insert_vertex(vertex_id);
    } else {
      eprintln!("Face with id {} not found", face_id);
    }
  }

  /**
  * Get flattened vertices from the BREP object
  */
  pub fn get_flattened_vertices(&self) -> Vec<Vector3> {
    self.vertices.iter().map(|v| v.position).collect()
  }

  // ==================== HALF-EDGE OPERATIONS ====================
  
  /// Add a vertex to the mesh
  pub fn add_vertex(&mut self, position: Vector3) -> u32 {
    let id = self.next_vertex_id();
    let vertex = Vertex::new(id, position);
    self.vertices.push(vertex);
    id
  }
  
  /// Add an edge connecting two vertices
  pub fn add_edge(&mut self, v1: u32, v2: u32) -> u32 {
    let id = self.next_edge_id();
    let edge = Edge::new(id, v1, v2);
    self.edges.push(edge);
    id
  }
  
  /// Add a half-edge
  pub fn add_halfedge(&mut self, vertex: u32) -> u32 {
    let id = self.next_halfedge_id();
    let halfedge = HalfEdge::new(id, vertex);
    self.halfedges.push(halfedge);
    id
  }
  
  /// Add a face
  pub fn add_face(&mut self) -> u32 {
    let id = self.next_face_id();
    let face = Face::new_with_halfedge(id, 0); // Will be set later
    self.faces.push(face);
    id
  }
  
  /// Create a complete edge with two half-edges
  pub fn create_edge_with_halfedges(&mut self, v1: u32, v2: u32) -> (u32, u32, u32) {
    // Create the edge
    let edge_id = self.add_edge(v1, v2);
    
    // Create two half-edges
    let he1_id = self.add_halfedge(v2); // Points to v2
    let he2_id = self.add_halfedge(v1); // Points to v1
    
    // Set up twin relationship - handle mutably one at a time
    if let Some(he1) = self.halfedges.iter_mut().find(|he| he.id == he1_id) {
      he1.set_twin(he2_id);
      he1.set_edge(edge_id);
    }
    if let Some(he2) = self.halfedges.iter_mut().find(|he| he.id == he2_id) {
      he2.set_twin(he1_id);
      he2.set_edge(edge_id);
      he2.set_boundary(); // Mark as boundary initially
    }
    
    // Link edge to half-edges
    if let Some(edge) = self.edges.iter_mut().find(|e| e.id == edge_id) {
      edge.set_halfedge1(he1_id);
      edge.set_halfedge2(he2_id);
    }
    
    (edge_id, he1_id, he2_id)
  }
  
  /// Link half-edges in a face loop
  pub fn link_halfedges_in_loop(&mut self, halfedge_ids: &[u32]) {
    let n = halfedge_ids.len();
    if n < 3 { return; } // Need at least 3 half-edges for a face
    
    for i in 0..n {
      let current_id = halfedge_ids[i];
      let next_id = halfedge_ids[(i + 1) % n];
      let prev_id = halfedge_ids[(i + n - 1) % n];
      
      if let Some(he) = self.halfedges.iter_mut().find(|he| he.id == current_id) {
        he.set_next(next_id);
        he.set_prev(prev_id);
      }
    }
  }
  
  /// Set face for a loop of half-edges
  pub fn set_face_for_halfedge_loop(&mut self, halfedge_ids: &[u32], face_id: u32) {
    for &he_id in halfedge_ids {
      if let Some(he) = self.halfedges.iter_mut().find(|he| he.id == he_id) {
        he.set_face(face_id);
      }
    }
    
    // Set the face's boundary half-edge to the first one
    if let (Some(first_he_id), Some(face)) = (
      halfedge_ids.first(),
      self.faces.iter_mut().find(|f| f.id == face_id)
    ) {
      face.set_halfedge(*first_he_id);
    }
  }
  
  /// Get vertices of a face using half-edge traversal
  pub fn get_face_vertices_via_halfedges(&self, face_id: u32) -> Vec<Vector3> {
    let mut vertices = Vec::new();
    
    if let Some(face) = self.faces.iter().find(|f| f.id == face_id) {
      if let Some(start_he_id) = face.get_halfedge() {
        let mut current_he_id = start_he_id;
        
        // Traverse the half-edge loop
        loop {
          if let Some(he) = self.halfedges.iter().find(|he| he.id == current_he_id) {
            // Get the target vertex
            if let Some(vertex) = self.vertices.iter().find(|v| v.id == he.vertex) {
              vertices.push(vertex.position);
            }
            
            // Move to next half-edge
            if let Some(next_id) = he.next {
              current_he_id = next_id;
              if current_he_id == start_he_id {
                break; // Completed the loop
              }
            } else {
              break; // No next half-edge
            }
          } else {
            break; // Half-edge not found
          }
        }
      }
    }
    
    vertices
  }
  
  /// Update vertex half-edge reference
  pub fn update_vertex_halfedge_reference(&mut self, vertex_id: u32, halfedge_id: u32) {
    if let Some(vertex) = self.vertices.iter_mut().find(|v| v.id == vertex_id) {
      vertex.set_halfedge(halfedge_id);
    }
  }
  
  /// Mark half-edge as boundary
  pub fn mark_halfedge_as_boundary(&mut self, halfedge_id: u32) {
    if let Some(he) = self.halfedges.iter_mut().find(|h| h.id == halfedge_id) {
      he.set_boundary();
    }
  }
  
  /// Find or create edge with half-edges
  pub fn find_or_create_edge_with_halfedges(&mut self, v1: u32, v2: u32) -> Result<(u32, u32, u32), String> {
    // Check if edge already exists
    if let Some(edge) = self.edges.iter().find(|e| (e.v1 == v1 && e.v2 == v2) || (e.v1 == v2 && e.v2 == v1)) {
      // Edge exists, return its half-edges
      if let (Some(he1), Some(he2)) = (edge.halfedge1, edge.halfedge2) {
        // Determine which half-edge goes from v1 to v2
        if let Some(he1_data) = self.halfedges.iter().find(|h| h.id == he1) {
          if let Some(twin_id) = he1_data.twin {
            if let Some(twin_data) = self.halfedges.iter().find(|h| h.id == twin_id) {
              if twin_data.vertex == v1 && he1_data.vertex == v2 {
                return Ok((edge.id, he1, he2));
              } else if twin_data.vertex == v2 && he1_data.vertex == v1 {
                return Ok((edge.id, he2, he1));
              }
            }
          }
        }
      }
    }
    
    // Edge doesn't exist, create it
    Ok(self.create_edge_with_halfedges(v1, v2))
  }
  
  /// Create a triangle face with proper half-edge connectivity
  pub fn create_triangle_face(&mut self, v1_pos: Vector3, v2_pos: Vector3, v3_pos: Vector3) -> u32 {
    // Add vertices
    let v1_id = self.add_vertex(v1_pos);
    let v2_id = self.add_vertex(v2_pos);
    let v3_id = self.add_vertex(v3_pos);
    
    // Create edges and half-edges
    let (_edge1_id, he1_id, he1_twin_id) = self.create_edge_with_halfedges(v1_id, v2_id);
    let (_edge2_id, he2_id, he2_twin_id) = self.create_edge_with_halfedges(v2_id, v3_id);
    let (_edge3_id, he3_id, he3_twin_id) = self.create_edge_with_halfedges(v3_id, v1_id);
    
    // Create face
    let face_id = self.add_face();
    
    // Link half-edges in loop (he1 -> he2 -> he3 -> he1)
    self.link_halfedges_in_loop(&[he1_id, he2_id, he3_id]);
    
    // Assign face to half-edges
    self.set_face_for_halfedge_loop(&[he1_id, he2_id, he3_id], face_id);
    
    // Update vertex half-edge references
    self.update_vertex_halfedge_reference(v1_id, he1_id);
    self.update_vertex_halfedge_reference(v2_id, he2_id);
    self.update_vertex_halfedge_reference(v3_id, he3_id);
    
    // Mark twin half-edges as boundary (external face)
    self.mark_halfedge_as_boundary(he1_twin_id);
    self.mark_halfedge_as_boundary(he2_twin_id);
    self.mark_halfedge_as_boundary(he3_twin_id);
    
    face_id
  }
  
  /// Create a quad face with proper half-edge connectivity
  pub fn create_quad_face(&mut self, v1_pos: Vector3, v2_pos: Vector3, v3_pos: Vector3, v4_pos: Vector3) -> u32 {
    // Add vertices
    let v1_id = self.add_vertex(v1_pos);
    let v2_id = self.add_vertex(v2_pos);
    let v3_id = self.add_vertex(v3_pos);
    let v4_id = self.add_vertex(v4_pos);
    
    // Create edges and half-edges
    let (_edge1_id, he1_id, he1_twin_id) = self.create_edge_with_halfedges(v1_id, v2_id);
    let (_edge2_id, he2_id, he2_twin_id) = self.create_edge_with_halfedges(v2_id, v3_id);
    let (_edge3_id, he3_id, he3_twin_id) = self.create_edge_with_halfedges(v3_id, v4_id);
    let (_edge4_id, he4_id, he4_twin_id) = self.create_edge_with_halfedges(v4_id, v1_id);
    
    // Create face
    let face_id = self.add_face();
    
    // Link half-edges in loop (he1 -> he2 -> he3 -> he4 -> he1)
    self.link_halfedges_in_loop(&[he1_id, he2_id, he3_id, he4_id]);
    
    // Assign face to half-edges
    self.set_face_for_halfedge_loop(&[he1_id, he2_id, he3_id, he4_id], face_id);
    
    // Update vertex half-edge references
    self.update_vertex_halfedge_reference(v1_id, he1_id);
    self.update_vertex_halfedge_reference(v2_id, he2_id);
    self.update_vertex_halfedge_reference(v3_id, he3_id);
    self.update_vertex_halfedge_reference(v4_id, he4_id);
    
    // Mark twin half-edges as boundary
    self.mark_halfedge_as_boundary(he1_twin_id);
    self.mark_halfedge_as_boundary(he2_twin_id);
    self.mark_halfedge_as_boundary(he3_twin_id);
    self.mark_halfedge_as_boundary(he4_twin_id);
    
    face_id
  }
  
  /// Build half-edge structure from existing face indices (legacy compatibility)
  pub fn build_halfedges_from_face(&mut self, face_id: u32) -> Result<(), String> {
    // Find the face
    let face_indices = if let Some(face) = self.faces.iter().find(|f| f.id == face_id) {
      face.face_indices.clone()
    } else {
      return Err(format!("Face {} not found", face_id));
    };
    
    if face_indices.len() < 3 {
      return Err("Face must have at least 3 vertices".to_string());
    }
    
    let mut halfedge_ids = Vec::new();
    let mut twin_halfedge_ids = Vec::new();
    
    // Create half-edges for each edge in the face
    for i in 0..face_indices.len() {
      let v1_id = face_indices[i];
      let v2_id = face_indices[(i + 1) % face_indices.len()];
      
      // Find or create edge with half-edges
      let edge_result = self.find_or_create_edge_with_halfedges(v1_id, v2_id);
      let (_edge_id, he_id, twin_id) = match edge_result {
        Ok(ids) => ids,
        Err(e) => return Err(e),
      };
      
      halfedge_ids.push(he_id);
      twin_halfedge_ids.push(twin_id);
    }
    
    // Link half-edges in loop
    self.link_halfedges_in_loop(&halfedge_ids);
    
    // Link boundary half-edges
    self.link_boundary_halfedges(&twin_halfedge_ids);
    
    // Assign face to half-edges
    self.set_face_for_halfedge_loop(&halfedge_ids, face_id);
    
    // Update vertex references
    for (i, &vertex_id) in face_indices.iter().enumerate() {
      self.update_vertex_halfedge_reference(vertex_id, halfedge_ids[i]);
    }
    
    Ok(())
  }
  
  /// Create a polygon face from vertex indices
  pub fn create_polygon_face_from_indices(&mut self, vertex_indices: &[u32]) -> Result<u32, String> {
    if vertex_indices.len() < 3 {
      return Err("Polygon must have at least 3 vertices".to_string());
    }
    
    let mut halfedge_ids = Vec::new();
    let mut twin_halfedge_ids = Vec::new();
    
    // Create half-edges for each edge in the polygon
    for i in 0..vertex_indices.len() {
      let v1_id = vertex_indices[i];
      let v2_id = vertex_indices[(i + 1) % vertex_indices.len()];
      
      // Create edge with half-edges
      let (_edge_id, he_id, twin_id) = self.create_edge_with_halfedges(v1_id, v2_id);
      halfedge_ids.push(he_id);
      twin_halfedge_ids.push(twin_id);
    }
    
    // Create face
    let face_id = self.add_face();
    
    // Link half-edges in loop (interior)
    self.link_halfedges_in_loop(&halfedge_ids);
    
    // Link boundary half-edges (twins) in reverse order
    self.link_boundary_halfedges(&twin_halfedge_ids);
    
    // Assign face to half-edges
    self.set_face_for_halfedge_loop(&halfedge_ids, face_id);
    
    // Update vertex references
    for (i, &vertex_id) in vertex_indices.iter().enumerate() {
      self.update_vertex_halfedge_reference(vertex_id, halfedge_ids[i]);
    }
    
    Ok(face_id)
  }
  
  /// Link boundary half-edges in proper order (reverse of face loop)
  pub fn link_boundary_halfedges(&mut self, twin_halfedge_ids: &[u32]) {
    let n = twin_halfedge_ids.len();
    if n < 3 { return; }
    
    // Link twins in reverse order to maintain consistent orientation
    for i in 0..n {
      let current_id = twin_halfedge_ids[i];
      // Next boundary half-edge is the previous one in the array (reverse order)
      let next_id = twin_halfedge_ids[(i + n - 1) % n];
      let prev_id = twin_halfedge_ids[(i + 1) % n];
      
      if let Some(he) = self.halfedges.iter_mut().find(|he| he.id == current_id) {
        he.set_next(next_id);
        he.set_prev(prev_id);
      }
    }
  }

  /// Fix half-edges with missing next/prev connections
  pub fn fix_incomplete_halfedges(&mut self) -> u32 {
    let mut fixed_count = 0;
    
    // Find all half-edges with missing next/prev
    let mut incomplete_halfedges = Vec::new();
    for he in &self.halfedges {
      if he.next.is_none() || he.prev.is_none() {
        incomplete_halfedges.push(he.id);
      }
    }
    
    // Group half-edges by face and fix connectivity
    let mut face_halfedges: HashMap<Option<u32>, Vec<u32>> = HashMap::new();
    
    for he in &self.halfedges {
      let face_key = he.face;
      face_halfedges.entry(face_key).or_insert_with(Vec::new).push(he.id);
    }
    
    // Fix each face's half-edge loop
    for (face_id, halfedge_ids) in face_halfedges {
      if halfedge_ids.len() >= 3 {
        if face_id.is_some() {
          // Interior face - link in order
          self.link_halfedges_in_loop(&halfedge_ids);
        } else {
          // Boundary face - link in reverse order
          self.link_boundary_halfedges(&halfedge_ids);
        }
        fixed_count += halfedge_ids.len() as u32;
      }
    }
    
    fixed_count
  }

  /// Validate half-edge data structure integrity
  pub fn validate_halfedge_structure(&self) -> Vec<String> {
    let mut errors = Vec::new();
    
    // Check for null next/prev references
    for he in &self.halfedges {
      if he.next.is_none() {
        errors.push(format!("Half-edge {} has null next reference", he.id));
      }
      if he.prev.is_none() {
        errors.push(format!("Half-edge {} has null prev reference", he.id));
      }
    }
    
    // Check half-edge twin relationships
    for he in &self.halfedges {
      if let Some(twin_id) = he.twin {
        if let Some(twin) = self.halfedges.iter().find(|h| h.id == twin_id) {
          if twin.twin != Some(he.id) {
            errors.push(format!("Half-edge {} twin relationship broken with {}", he.id, twin_id));
          }
        } else {
          errors.push(format!("Half-edge {} references non-existent twin {}", he.id, twin_id));
        }
      } else {
        errors.push(format!("Half-edge {} has no twin", he.id));
      }
    }
    
    // Check next/prev relationships
    for he in &self.halfedges {
      if let Some(next_id) = he.next {
        if let Some(next_he) = self.halfedges.iter().find(|h| h.id == next_id) {
          if next_he.prev != Some(he.id) {
            errors.push(format!("Half-edge {} next/prev relationship broken with {}", he.id, next_id));
          }
        } else {
          errors.push(format!("Half-edge {} references non-existent next half-edge {}", he.id, next_id));
        }
      }
    }
    
    // Check vertex half-edge references
    for vertex in &self.vertices {
      if let Some(he_id) = vertex.halfedge {
        if let Some(he) = self.halfedges.iter().find(|h| h.id == he_id) {
          // The half-edge should originate from this vertex (be outgoing)
          // This means the twin should point to this vertex
          if let Some(twin_id) = he.twin {
            if let Some(twin) = self.halfedges.iter().find(|h| h.id == twin_id) {
              if twin.vertex != vertex.id {
                errors.push(format!("Vertex {} half-edge reference incorrect", vertex.id));
              }
            }
          }
        } else {
          errors.push(format!("Vertex {} references non-existent half-edge {}", vertex.id, he_id));
        }
      }
    }
    
    errors
  }

  /**
   * Get vertices and holes by face ID
   * @param face_id - The ID of the face to get vertices and holes for
   * @returns (Vec<Vector3>, Vec<Vec<Vector3>>) - A tuple containing face vertices and hole vertices
   */
  pub fn get_vertices_and_holes_by_face_id(&self, face_id: u32) -> (Vec<Vector3>, Vec<Vec<Vector3>>) {
    // Try half-edge method first
    let face_vertices = self.get_face_vertices_via_halfedges(face_id);
    if !face_vertices.is_empty() {
      // Get holes via half-edges if available
      let mut holes_vertices = Vec::new();
      if let Some(face) = self.faces.iter().find(|f| f.id == face_id) {
        for &hole_he_id in face.get_holes() {
          let hole_verts = self.get_hole_vertices_via_halfedges(hole_he_id);
          holes_vertices.push(hole_verts);
        }
      }
      return (face_vertices, holes_vertices);
    }
    
    // Fallback to legacy method
    if let Some(face) = self.faces.iter().find(|f| f.id == face_id) {
      // Get the main face vertices
      let mut face_vertices = Vec::new();
      for &vertex_index in &face.face_indices {
        if let Some(vertex) = self.vertices.get(vertex_index as usize) {
          face_vertices.push(vertex.position);
        }
      }
      
      let mut holes_vertices = Vec::new();
  
      if self.holes.len() > 0 {
        for &hole_start_index in &self.holes {
          let mut hole_vertices = Vec::new();
          let next_hole_start_index = self.holes.iter()
            .filter(|&&idx| idx > hole_start_index)
            .min()
            .cloned()
            .unwrap_or(self.vertices.len() as u32);
          
          for i in hole_start_index..next_hole_start_index {
            if let Some(vertex) = self.vertices.get(i as usize) {
              hole_vertices.push(vertex.position);
            }
          }
          holes_vertices.push(hole_vertices);
        }
      }

      (face_vertices, holes_vertices)
    } else {
      // Face not found, return empty vectors
      eprintln!("Face with id {} not found", face_id);
      (Vec::new(), Vec::new())
    }
  }
  
  /// Get vertices of a hole using half-edge traversal
  fn get_hole_vertices_via_halfedges(&self, hole_he_id: u32) -> Vec<Vector3> {
    let mut vertices = Vec::new();
    let mut current_he_id = hole_he_id;
    
    // Traverse the hole boundary
    loop {
      if let Some(he) = self.halfedges.iter().find(|he| he.id == current_he_id) {
        // Get the target vertex
        if let Some(vertex) = self.vertices.iter().find(|v| v.id == he.vertex) {
          vertices.push(vertex.position);
        }
        
        // Move to next half-edge
        if let Some(next_id) = he.next {
          current_he_id = next_id;
          if current_he_id == hole_he_id {
            break; // Completed the hole loop
          }
        } else {
          break; // No next half-edge
        }
      } else {
        break; // Half-edge not found
      }
    }
    
    vertices
  }
}
