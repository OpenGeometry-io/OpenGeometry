pub mod geometry {
  pub mod basegeometry;
  pub mod triangle;
}

pub mod operations {
  pub mod triangulate;
  pub mod windingsort;
  pub mod extrude;
}

pub mod utility {
  pub mod geometry;
  pub mod bgeometry;
}

pub mod primitives {
  pub mod arc;
  pub mod line;
  pub mod polyline;
  pub mod rectangle;
  pub mod polygon;
  pub mod cylinder;
  pub mod cuboid;
}

pub mod brep;

use wasm_bindgen::prelude::*;
use uuid::Uuid;
use openmaths::Vector3;

/// Test function to validate half-edge data structure implementation
/// Creates a simple triangle using the new high-level methods
#[wasm_bindgen]
pub fn test_halfedge_triangle() -> String {
  use brep::Brep;
  
  let mut brep = Brep::new(Uuid::new_v4());
  
  // Create a triangle using the high-level method
  let face_id = brep.create_triangle_face(
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(1.0, 0.0, 0.0),
    Vector3::new(0.5, 1.0, 0.0)
  );
  
  // Validate the structure
  let validation_errors = brep.validate_halfedge_structure();
  
  // Get face vertices using half-edge traversal
  let face_vertices = brep.get_face_vertices_via_halfedges(face_id);
  
  // Build result message with detailed connectivity info
  let mut result = if validation_errors.is_empty() {
    format!(
      "Half-edge triangle test PASSED!\n\
      Vertices: {}\n\
      Edges: {}\n\
      Half-edges: {}\n\
      Faces: {}\n\
      Face vertices from half-edge traversal: {} vertices\n\
      Structure validation: CLEAN\n\n",
      brep.get_vertex_count(),
      brep.get_edge_count(),
      brep.get_halfedge_count(),
      brep.get_face_count(),
      face_vertices.len()
    )
  } else {
    format!(
      "Half-edge triangle test FAILED!\n\
      Validation errors: {}\n\
      Errors: {:?}\n\n",
      validation_errors.len(),
      validation_errors
    )
  };
  
  // Add detailed half-edge connectivity information
  result.push_str("Half-edge Details:\n");
  for (i, he) in brep.halfedges.iter().enumerate() {
    result.push_str(&format!(
      "HE{}: id={}, vertex={}, twin={:?}, next={:?}, prev={:?}, edge={:?}, face={:?}, boundary={}\n",
      i, he.id, he.vertex, he.twin, he.next, he.prev, he.edge, he.face, he.is_boundary
    ));
  }
  
  result.push_str("\nVertex Details:\n");
  for (i, v) in brep.vertices.iter().enumerate() {
    result.push_str(&format!(
      "V{}: id={}, pos=({:.2}, {:.2}, {:.2}), halfedge={:?}\n",
      i, v.id, v.position.x, v.position.y, v.position.z, v.halfedge
    ));
  }
  
  result
}

/// Test function for creating a quad with half-edges
#[wasm_bindgen]
pub fn test_halfedge_quad() -> String {
  use brep::Brep;
  
  let mut brep = Brep::new(Uuid::new_v4());
  
  // Create a quad using the high-level method
  let face_id = brep.create_quad_face(
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(1.0, 0.0, 0.0),
    Vector3::new(1.0, 1.0, 0.0),
    Vector3::new(0.0, 1.0, 0.0)
  );
  
  // Validate the structure
  let validation_errors = brep.validate_halfedge_structure();
  
  // Get face vertices using half-edge traversal
  let face_vertices = brep.get_face_vertices_via_halfedges(face_id);
  
  let result = if validation_errors.is_empty() {
    format!(
      "Half-edge quad test PASSED!\n\
      Vertices: {}\n\
      Edges: {}\n\
      Half-edges: {}\n\
      Faces: {}\n\
      Face vertices from half-edge traversal: {} vertices\n\
      Structure validation: CLEAN",
      brep.get_vertex_count(),
      brep.get_edge_count(),
      brep.get_halfedge_count(),
      brep.get_face_count(),
      face_vertices.len()
    )
  } else {
    format!(
      "Half-edge quad test FAILED!\n\
      Validation errors: {}\n\
      Errors: {:?}",
      validation_errors.len(),
      validation_errors
    )
  };
  
  result
}

/// Test function for cuboid with half-edges
#[wasm_bindgen]
pub fn test_halfedge_cuboid() -> String {
  use primitives::cuboid::OGCuboid;
  
  let mut cuboid = OGCuboid::new("test-cuboid".to_string());
  cuboid.set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0);
  
  // Get the B-Rep and check half-edge connectivity
  let brep_serialized = cuboid.get_brep_serialized();
  let mut brep: brep::Brep = serde_json::from_str(&brep_serialized).unwrap();
  
  // Fix any incomplete half-edges
  let fixed_count = brep.fix_incomplete_halfedges();
  
  // Validate the structure
  let validation_errors = brep.validate_halfedge_structure();
  
  // Count null next/prev references
  let mut null_next_count = 0;
  let mut null_prev_count = 0;
  for he in &brep.halfedges {
    if he.next.is_none() { null_next_count += 1; }
    if he.prev.is_none() { null_prev_count += 1; }
  }
  
  // Build result message
  let mut result = format!(
    "Half-edge cuboid test results:\n\
    Vertices: {}\n\
    Edges: {}\n\
    Half-edges: {}\n\
    Faces: {}\n\
    Fixed half-edges: {}\n\
    Null next references: {}\n\
    Null prev references: {}\n\
    Validation errors: {}\n",
    brep.get_vertex_count(),
    brep.get_edge_count(),
    brep.get_halfedge_count(),
    brep.get_face_count(),
    fixed_count,
    null_next_count,
    null_prev_count,
    validation_errors.len()
  );
  
  if !validation_errors.is_empty() {
    result.push_str("\nErrors:\n");
    for error in &validation_errors {
      result.push_str(&format!("- {}\n", error));
    }
  }
  
  if validation_errors.is_empty() && null_next_count == 0 && null_prev_count == 0 {
    result.push_str("\n✅ PASSED: Half-edges are properly connected!");
  } else {
    result.push_str("\n❌ FAILED: Half-edges have connectivity issues!");
  }
  
  result
}

// v0.3.0
// mod brep_ds {
//   mod vertex;
//   // mod halfedge;
//   mod edge;
//   mod face;
//   pub mod brep;
// }

