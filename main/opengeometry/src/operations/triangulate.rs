use crate::geometry::{basegeometry::BaseGeometry, triangle::Triangle};
use crate::operations::windingsort;
use crate::brep::Brep;
use std::collections::HashMap;
use openmaths::Vector3;

// Helper functions for boolean operations
/// Ensures a Brep has triangulation data
pub fn ensure_triangulated(brep: &mut Brep) -> Result<(), String> {
    if brep.triangulation.is_none() {
        return Err("Brep triangulation is required but not present. Call generate_geometry() first.".to_string());
    }
    Ok(())
}

/// Creates triangulation from face data
pub fn triangulate_brep_faces(brep: &mut Brep) -> Result<(), String> {
    let mut all_triangles = Vec::new();
    
    for face in &brep.faces {
        let face_vertices = brep.get_vertices_by_face_id(face.id);
        if face_vertices.len() < 3 {
            continue; // Skip degenerate faces
        }
        
        let triangle_indices = triangulate_polygon_with_holes(&face_vertices, &vec![]);
        
        // Convert indices to actual triangles
        for triangle_idx in triangle_indices {
            if triangle_idx[0] < face_vertices.len() &&
               triangle_idx[1] < face_vertices.len() &&
               triangle_idx[2] < face_vertices.len() {
                let triangle = Triangle::new_with_vertices(
                    face_vertices[triangle_idx[0]],
                    face_vertices[triangle_idx[1]],
                    face_vertices[triangle_idx[2]],
                );
                all_triangles.push(triangle);
            }
        }
    }
    
    brep.triangulation = Some(all_triangles);
    Ok(())
}

// 00000000000000

pub fn triangulate_polygon_with_holes(
    face_vertices: &Vec<Vector3>,
    holes: &Vec<Vec<Vector3>>,
) -> Vec<[usize; 3]> {
    if face_vertices.len() < 3 {
        return Vec::new();
    }

    // --- 1. Projection to 2D ---
    // First, determine the best 2D plane to project onto by finding the
    // dominant axis of the face's normal.
    let normal = calculate_normal(face_vertices);

    let (axis_u, axis_v) = if normal.z.abs() > normal.x.abs() && normal.z.abs() > normal.y.abs() {
        // Project to XY plane
        (0, 1) // Corresponds to (x, y)
    } else if normal.x.abs() > normal.y.abs() {
        // Project to YZ plane
        (1, 2) // Corresponds to (y, z)
    } else {
        // Project to XZ plane
        (0, 2) // Corresponds to (x, z)
    };

    // --- 2. Flatten Data for earcutr ---
    // earcutr needs a flat list of 2D coordinates and a list of indices
    // where the holes begin.

    let mut vertices_2d = Vec::with_capacity((face_vertices.len() + holes.iter().map(|h| h.len()).sum::<usize>()) * 2);
    let mut hole_indices = Vec::with_capacity(holes.len());

    // Add outer loop vertices
    for v in face_vertices {
        let coord_u = match axis_u {
            0 => v.x,
            1 => v.y,
            2 => v.z,
            _ => unreachable!("Invalid axis_u"),
        };
        let coord_v = match axis_v {
            0 => v.x,
            1 => v.y,
            2 => v.z,
            _ => unreachable!("Invalid axis_v"),
        };
        vertices_2d.push(coord_u);
        vertices_2d.push(coord_v);
    }

    // Add hole vertices
    if !holes.is_empty() {
        let mut current_index = face_vertices.len();
        for hole in holes {
            hole_indices.push(current_index);
            for v in hole {
                let coord_u = match axis_u {
                    0 => v.x,
                    1 => v.y,
                    2 => v.z,
                    _ => unreachable!("Invalid axis_u"),
                };
                let coord_v = match axis_v {
                    0 => v.x,
                    1 => v.y,
                    2 => v.z,
                    _ => unreachable!("Invalid axis_v"),
                };
                vertices_2d.push(coord_u);
                vertices_2d.push(coord_v);
                current_index += 1;
            }
        }
    }

    // --- 3. Run Earcut Algorithm ---
    let triangle_indices = earcutr::earcut(&vertices_2d, &hole_indices, 2);

    // --- 4. Reshape the Result ---
    // The result is a flat list of indices. Group them into triangles.
    triangle_indices
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect()
}

/**
 * Calculates the average normal of a polygon.
 * This is used to determine the best projection plane.
 */
fn calculate_normal(vertices: &[Vector3]) -> Vector3 {
    if vertices.len() < 3 {
        return Vector3::new(0.0, 0.0, 1.0); // Default to Z-axis
    }
    let mut normal = Vector3::new(0.0, 0.0, 0.0);
    for i in 0..vertices.len() {
        let p1 = &vertices[i];
        let p2 = &vertices[(i + 1) % vertices.len()];
        normal.x += (p1.y - p2.y) * (p1.z + p2.z);
        normal.y += (p1.z - p2.z) * (p1.x + p2.x);
        normal.z += (p1.x - p2.x) * (p1.y + p2.y);
    }
    normal.normalize();
    normal
}

// 00000000000000

pub fn ear_triangle_test(
  vertices: HashMap<u32, Vec<f64>>,
  a_index: u32,
  b_index: u32,
  c_index: u32,
) -> bool {
  let point_a = Vector3::new(
    vertices[&(a_index)][0],
    vertices[&(a_index)][1],
    vertices[&(a_index)][2]
  );
  let point_b = Vector3::new(
    vertices[&(b_index)][0],
    vertices[&(b_index)][1],
    vertices[&(b_index)][2]
  );
  let point_c = Vector3::new(
    vertices[&(c_index)][0],
    vertices[&(c_index)][1],
    vertices[&(c_index)][2]
  );
  let ba = point_b.clone().subtract(&point_a);
  let bc = point_b.clone().subtract(&point_c);
  let cross_product = ba.cross(&bc);

  if cross_product.y < 0.0 {
    return false;
  }

  let mut triangle = Triangle::new();
  triangle.set_vertices(point_a, point_b, point_c);

  for (i, vertex) in vertices.iter() {
    if *i != a_index && *i != b_index && *i != c_index {
      let p = Vector3::new(vertex[0], vertex[1], vertex[2]);
      if triangle.is_point_in_triangle(p) {
        return false;
      }
    }
  }

  true
}

// Accepting Vertices in CCW order
pub fn tricut(polygon_vertices: Vec<Vector3>) -> Vec<Vec<u32>> {
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

  let vertices;

  if geom_buf.ccw {
    vertices = raw_vertices;
  } else {
    vertices = windingsort::ccw_test(raw_vertices.clone());
  }
  // let mut triangles_vertices: Vec<f64> = Vec::new();
  let tri_indices = tricut(vertices);
  
  tri_indices
}

//
// Triangule by faces and vertices
//
pub fn triangulate_polygon_by_face(face: Vec<Vector3>) -> Vec<Vec<u32>> {
  let raw_vertices = face.clone();
  let ccw_vertices = windingsort::ccw_test(raw_vertices.clone());

  // print javascript console log of ccw_vertices
  for _vertex in &ccw_vertices {
    // web_sys::console::log_1(&format!("Vertex: ({}, {}, {})", vertex.x, vertex.y, vertex.z).into());
  }  

  // let mut triangles_vertices: Vec<f64> = Vec::new();
  let tri_indices = tricut(ccw_vertices);
  
  tri_indices
}

pub struct FlattenData {
  pub vertices: Vec<f64>,
  pub holes: Vec<u32>,
  pub dimension: u32,
}

pub fn flatten_buffer_geometry(mut geom_buf: BaseGeometry) -> FlattenData {
  let mut vertices: Vec<f64> = Vec::new();
  let mut holes: Vec<u32> = Vec::new();

  let dimension: u32 = 3;

  let mut current_index = 0;

  for vertex in geom_buf.get_vertices() {
    vertices.push(vertex.x);
    vertices.push(vertex.y);
    vertices.push(vertex.z);

    current_index += 1;
  }

  // Do we check for clockwise or counterclockwise here?
  for hole in geom_buf.get_holes() {
    for vertex in &hole {
      vertices.push(vertex.x);
      vertices.push(vertex.y);
      vertices.push(vertex.z);
    }
    
    holes.push(current_index);

    current_index += hole.len() as u32;
  }

  FlattenData {
    vertices,
    holes,
    dimension,
  }
}

/**
 * Find Left Most Point in Given Polygon
 */
// pub fn find_left_most_point(flat_data_vertices: Vec<f64>, start: u32, end: u32) -> Vector3 {
//   let mut left_most_index = start;
//   let mut left_most_x = flat_data_vertices[start as usize * 3];

//   for i in start..end {
//     let x = flat_data_vertices[i as usize * 3];
//     if x < left_most_x {
//       left_most_x = x;
//       left_most_index = i;
//     }
//   }

//   Vector3::new(
//     flat_data_vertices[left_most_index as usize * 3],
//     flat_data_vertices[left_most_index as usize * 3 + 1],
//     flat_data_vertices[left_most_index as usize * 3 + 2]
//   )
// }

/**
 * Find Right Most Point in Given Polygon or Hole
 */
pub fn find_right_most_point_index(flat_data_vertices: Vec<f64>, start: u32, end: u32) -> u32 {
  let mut i = start;
  let mut right_most_index = start;
  
  let mut right_most_x = flat_data_vertices[start as usize];

  while i < end {
    let x = flat_data_vertices[i as usize];
    if x > right_most_x {
      right_most_x = x;
      right_most_index = i;
    }

    i += 3;
  }
  
  right_most_index
}

pub fn check_vertex_collision_with_flat_vertices(
  flat_data_vertices: Vec<f64>,
  right_max_point: Vector3,
  start: u32,
  end: u32
) -> Vec<Vec<u32>> {
  let mut i = start;
  let mut potential_edge: Vec<Vec<u32>> = Vec::new();

  // Traverse All The Edges
  while i < end {
    // This while loop will check from A to B, and B to C
    if (i + 6) < end {
      let _x = flat_data_vertices[i as usize];
      let _y = flat_data_vertices[i as usize + 1];
      let z = flat_data_vertices[i as usize + 2];

      let _x1 = flat_data_vertices[i as usize + 3];
      let _y1 = flat_data_vertices[i as usize + 4];
      let z1 = flat_data_vertices[i as usize + 5];

      // Check if z of A is more than z of right_max_point and z of B is less than z of right_max_point
      if z <= right_max_point.z && z1 >= right_max_point.z {
        let edge_index = vec![i, i + 3];
        potential_edge.push(edge_index);
      }
    } else {
      // This is for last edge
      let _x = flat_data_vertices[i as usize];
      let _y = flat_data_vertices[i as usize + 1];
      let z = flat_data_vertices[i as usize + 2];

      let _x1 = flat_data_vertices[start as usize];
      let _y1 = flat_data_vertices[start as usize + 1];
      let z1 = flat_data_vertices[start as usize + 2];

      // Check if z of A is more than z of right_max_point and z of B is less than z of right_max_point
      if z <= right_max_point.z && z1 >= right_max_point.z {
        let edge_index = vec![i, start];
        potential_edge.push(edge_index);
      }

      break;
    }

    i += 3;
  }

  // Step 4 - Cast the ray from the right_most_index to all potential edges and check if it intersects without obstacles
  let mut found_edge: Vec<Vec<u32>> = Vec::new();
  for edge in potential_edge {
    let a_index = edge[0];
    let b_index = edge[1];

    let x = flat_data_vertices[a_index as usize];
    let y = flat_data_vertices[a_index as usize + 1];
    let z = flat_data_vertices[a_index as usize + 2];
    let a = Vector3::new(x, y, z);

    let x1 = flat_data_vertices[b_index as usize];
    let y1 = flat_data_vertices[b_index as usize + 1];
    let z1 = flat_data_vertices[b_index as usize + 2];
    let b = Vector3::new(x1, y1, z1);

    // Check if the ray intersects with the edge and no obstacles
    let right_ray_from_vertex = Vector3::new(right_max_point.x + 1.0, right_max_point.y, right_max_point.z);
    let ray = right_ray_from_vertex.clone().subtract(&right_max_point);
    let edge_vector = b.clone().subtract(&a);
    let cross_product = ray.cross(&edge_vector);
    let cross_product_length = cross_product.dot(&cross_product);
    let edge_vector_length = edge_vector.dot(&edge_vector);
    let ray_length = ray.dot(&ray);
    let denominator = cross_product_length * edge_vector_length - ray_length * edge_vector_length;
    if denominator == 0.0 {
      continue; // Parallel lines
    }
    let t = (cross_product.dot(&edge_vector) * ray_length - cross_product.dot(&ray) * edge_vector_length) / denominator;
    let u = (cross_product.dot(&ray) * edge_vector_length - cross_product.dot(&edge_vector) * ray_length) / denominator;
    if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 {
      // The ray intersects the edge
      let f_edge = vec![a_index, b_index];
      found_edge.push(f_edge);
      break;
    }
  }

  found_edge
}


/**
 * Vertex Node for Tricut
 * This is used to store the vertex information and its connections in the triangulation process
 * is_hole: true if the vertex is part of a hole
 * is_hole_treated: true if the hole has been treated
 * hole_index: index of the hole in the list of holes, -1 if not a hole
 * next_index: index of the next vertex in the list
 * prev_index: index of the previous vertex in the list
 * treated: true if the vertex has been treated
 * index: index of the vertex in the main list
 * vertex: the vertex itself
 */
#[derive(Clone)]
pub struct VertexNodeTricut {
  pub vertex: Vector3,
  pub index: u32,
  pub treated: bool,
  pub is_hole: bool,
  pub is_hole_treated: bool,
  pub hole_index: i32,
  pub next_index: u32,
  pub prev_index: u32
}


impl VertexNodeTricut {
  pub fn new(vertex: Vector3, index: u32) -> Self {
    Self {
      vertex,
      index,
      treated: false,
      is_hole: false,
      is_hole_treated: false,
      hole_index: -1,
      next_index: 0,
      prev_index: 0
    }
  }
}

pub fn create_vertex_nodes(vertices: Vec<Vector3>, start: u32, end: u32, _is_hole: bool) -> Vec<VertexNodeTricut> {
  let mut vertex_nodes: Vec<VertexNodeTricut> = Vec::new();

  let mut index = start;
  let mut current_index = start;

  for vertex in vertices {
    let node = VertexNodeTricut::new(
      vertex,
      index
    );
    vertex_nodes.push(node);

    index += 3;
  }

  // Set the next and previous indices
  for i in 0..vertex_nodes.len() {
    let next_index = current_index + 3;
    let prev_index = current_index - 3;
    vertex_nodes[i].next_index = next_index;
    vertex_nodes[i].prev_index = prev_index;
    current_index += 3;
    if current_index >= end {
      vertex_nodes[i].next_index = start;
    }
    if current_index <= start {
      vertex_nodes[i].prev_index = end;
    }
  }

  // Set the is_hole and hole_index properties
  // if (is_hole) {
  //   for i in 0..vertex_nodes.len() {
  //     vertex_nodes[i].is_hole = true;
  //     vertex_nodes[i].hole_index = 0;
  //   }
  // } else {
  //   for i in 0..vertex_nodes.len() {
  //     vertex_nodes[i].is_hole = false;
  //     vertex_nodes[i].hole_index = -1;
  //   }
  // }

  vertex_nodes
}
