// Export out
pub mod vertex;
pub mod edge;
// pub mod halfedge;
pub mod face;

use openmaths::Vector3;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

// Import and re-export types
pub use vertex::Vertex;
pub use edge::Edge;
pub use face::Face;

#[derive(Clone, Serialize, Deserialize)]
pub struct Brep {
  pub id: Uuid,
  pub vertices: Vec<Vertex>,
  pub edges: Vec<Edge>,
  // pub halfedges: Vec<HalfEdge>,
  pub faces: Vec<Face>,
}

impl Brep {
  pub fn new(id: Uuid) -> Self {
    Brep {
      id,
      vertices: Vec::new(),
      edges: Vec::new(),
      // halfedges: Vec::new(),
      faces: Vec::new(),
    }
  }

  pub fn clear(&mut self) {
    self.vertices.clear();
    self.edges.clear();
    // self.halfedges.clear();
    self.faces.clear();
  }

  pub fn get_vertex_count(&self) -> u32 {
    self.vertices.len() as u32
  }

  pub fn get_edge_count(&self) -> u32 {
    self.edges.len() as u32
  }

  pub fn get_face_count(&self) -> u32 {
    self.faces.len() as u32
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

  /**
   * Get vertices and holes by face ID
   * @param face_id - The ID of the face to get vertices and holes for
   * @returns (Vec<Vector3>, Vec<Vec<Vector3>>) - A tuple containing face vertices and hole vertices
   */
  pub fn get_vertices_and_holes_by_face_id(&self, face_id: u32) -> (Vec<Vector3>, Vec<Vec<Vector3>>) {
    // Find the face by ID
    if let Some(face) = self.faces.iter().find(|f| f.id == face_id) {
      // Get the main face vertices
      let mut face_vertices = Vec::new();
      for &vertex_index in &face.face_indices {
        if let Some(vertex) = self.vertices.get(vertex_index as usize) {
          face_vertices.push(vertex.position);
        }
      }
      
      // For now, return empty holes vector since the current Face structure doesn't support holes
      // TODO: Implement proper hole support in Face structure when needed
      let holes_vertices = Vec::new();
      
      (face_vertices, holes_vertices)
    } else {
      // Face not found, return empty vectors
      eprintln!("Face with id {} not found", face_id);
      (Vec::new(), Vec::new())
    }
  }
}
