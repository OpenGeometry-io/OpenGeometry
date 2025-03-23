use crate::geometry::{self, basegeometry::BaseGeometry, triangle::Triangle};
use crate::operations::windingsort;
use std::collections::HashMap;
use crate::utility::openmath::Vector3D;

pub fn ear_triangle_test(
  vertices: HashMap<u32, Vec<f64>>,
  a_index: u32,
  b_index: u32,
  c_index: u32,
) -> bool {
  let point_a = Vector3D::create(
    vertices[&(a_index)][0],
    vertices[&(a_index)][1],
    vertices[&(a_index)][2]
  );
  let point_b = Vector3D::create(
    vertices[&(b_index)][0],
    vertices[&(b_index)][1],
    vertices[&(b_index)][2]
  );
  let point_c = Vector3D::create(
    vertices[&(c_index)][0],
    vertices[&(c_index)][1],
    vertices[&(c_index)][2]
  );
  let ba = point_b.subtract(&point_a);
  let bc = point_b.subtract(&point_c);
  let cross_product = ba.cross(&bc);

  if cross_product.y < 0.0 {
    return false;
  }

  let mut triangle = Triangle::new();
  triangle.set_vertices(point_a, point_b, point_c);

  for (i, vertex) in vertices.iter() {
    if *i != a_index && *i != b_index && *i != c_index {
      let p = Vector3D::create(vertex[0], vertex[1], vertex[2]);
      if triangle.is_point_in_triangle(p) {
        return false;
      }
    }
  }

  true
}

// Accepting Vertices in CCW order
pub fn tricut(polygon_vertices: Vec<Vector3D>) -> Vec<Vec<u32>> {
  let mut all_vertices: HashMap<u32, Vec<f64>> = HashMap::new();
  for (i, vertex) in polygon_vertices.iter().enumerate() {
    all_vertices.insert(i as u32, vec![vertex.x, vertex.y, vertex.z]);
  }

  let mut remaining_vertices: Vec<u32> = (0..all_vertices.len() as u32).collect();
  let mut triangle_indices: Vec<Vec<u32>> = Vec::new();
  
  while remaining_vertices.len() > 3 {
    let len = remaining_vertices.len();
    for i in 0..len {
      let a = remaining_vertices[(i + len - 1) % len];
      let b = remaining_vertices[i];
      let c = remaining_vertices[(i + 1) % len];
      
      if ear_triangle_test(all_vertices.clone(), a, b, c) {
        triangle_indices.push(vec![a, b, c]); // changed from vec![a, b, c] to vec![a, c, b]
        remaining_vertices.remove(i);
        break;
      }
    }
  }
  
  // Reverse the order for the last triangle as well
  triangle_indices.push(vec![
    remaining_vertices[0],
    remaining_vertices[1], // changed from [0, 1, 2] to [0, 2, 1]
    remaining_vertices[2],
  ]);

  triangle_indices
}


pub fn triangulate_polygon_buffer_geometry(geom_buf: BaseGeometry) -> Vec<Vec<u32>> {
  
  let raw_vertices = geom_buf.get_vertices().clone();
  let ccw_vertices = windingsort::ccw_test(raw_vertices.clone());

  // let mut triangles_vertices: Vec<f64> = Vec::new();
  let tri_indices = tricut(ccw_vertices);
  
  tri_indices
}


//
// Triangule by faces and vertices
//
pub fn triangulate_polygon_by_face(face: Vec<Vector3D>) -> Vec<Vec<u32>> {
  let raw_vertices = face.clone();
  let ccw_vertices = windingsort::ccw_test(raw_vertices.clone());

  // let mut triangles_vertices: Vec<f64> = Vec::new();
  let tri_indices = tricut(ccw_vertices);
  
  tri_indices
}

