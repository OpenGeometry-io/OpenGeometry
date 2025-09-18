use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys;

use crate::geometry::{basegeometry::BaseGeometry, triangle::Triangle};
use crate::operations::windingsort;
use std::collections::HashMap;
use openmaths::Vector3;

// Constants for robust triangulation
const EPSILON: f64 = 1e-10;

// Node structure for doubly linked list used in earcut algorithm
#[derive(Clone, Debug)]
pub struct Node {
    pub i: usize,        // vertex index in original input array
    pub x: f64,          // vertex x coordinate
    pub y: f64,          // vertex y coordinate
    pub z: f64,          // vertex z coordinate (for 3D support)
    pub prev: Option<usize>, // previous node index
    pub next: Option<usize>, // next node index
    pub z_order: u32,    // z-order curve value for spatial indexing
    pub prev_z: Option<usize>, // previous node in z-order
    pub next_z: Option<usize>, // next node in z-order
    pub steiner: bool,   // indicates whether this is a steiner point
    pub removed: bool,   // indicates whether this node has been removed
}

impl Node {
    pub fn new(i: usize, x: f64, y: f64, z: f64) -> Self {
        Node {
            i,
            x,
            y,
            z,
            prev: None,
            next: None,
            z_order: 0,
            prev_z: None,
            next_z: None,
            steiner: false,
            removed: false,
        }
    }
}

pub fn ear_triangle_test(
  vertices: HashMap<u32, Vec<f64>>,
  a_index: u32,
  b_index: u32,
  c_index: u32,
) -> bool {
  let mut point_a = Vector3::new(
    vertices[&(a_index)][0],
    vertices[&(a_index)][1],
    vertices[&(a_index)][2]
  );
  let mut point_b = Vector3::new(
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

  if (geom_buf.ccw) {
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
  for vertex in &ccw_vertices {
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
      let x = flat_data_vertices[i as usize];
      let y = flat_data_vertices[i as usize + 1];
      let z = flat_data_vertices[i as usize + 2];

      let x1 = flat_data_vertices[i as usize + 3];
      let y1 = flat_data_vertices[i as usize + 4];
      let z1 = flat_data_vertices[i as usize + 5];

      // Check if z of A is more than z of right_max_point and z of B is less than z of right_max_point
      if (z <= right_max_point.z && z1 >= right_max_point.z) {
        let edge_index = vec![i, i + 3];
        potential_edge.push(edge_index);
      }
    } else {
      // This is for last edge
      let x = flat_data_vertices[i as usize];
      let y = flat_data_vertices[i as usize + 1];
      let z = flat_data_vertices[i as usize + 2];

      let x1 = flat_data_vertices[start as usize];
      let y1 = flat_data_vertices[start as usize + 1];
      let z1 = flat_data_vertices[start as usize + 2];

      // Check if z of A is more than z of right_max_point and z of B is less than z of right_max_point
      if (z <= right_max_point.z && z1 >= right_max_point.z) {
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
    let a_point = Vector3::new(x, y, z);

    let x1 = flat_data_vertices[b_index as usize];
    let y1 = flat_data_vertices[b_index as usize + 1];
    let z1 = flat_data_vertices[b_index as usize + 2];
    let b_point = Vector3::new(x1, y1, z1);

    // Check if the ray intersects with the edge and no obstacles
    let right_ray_from_vertex = Vector3::new(right_max_point.x + 1.0, right_max_point.y, right_max_point.z);
    let ray = right_ray_from_vertex.clone().subtract(&right_max_point);
    let edge_vector = b_point.clone().subtract(&a_point);
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

pub fn create_vertex_nodes(vertices: Vec<Vector3>, start: u32, end: u32, is_hole: bool) -> Vec<VertexNodeTricut> {
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

// =======================
// ROBUST TRIANGULATION WITH HOLE SUPPORT
// Based on earcut algorithm by Mapbox
// =======================

/// Project 3D polygon to 2D for triangulation
fn project_3d_to_2d(data: &[f64], dim: usize) -> (Vec<f64>, bool) {
    if dim != 3 {
        return (data.to_vec(), false);
    }
    
    let num_vertices = data.len() / 3;
    if num_vertices < 3 {
        return (data.to_vec(), false);
    }
    
    // Calculate polygon normal using Newell's method
    let mut normal = [0.0, 0.0, 0.0];
    for i in 0..num_vertices {
        let j = (i + 1) % num_vertices;
        let curr = [data[i * 3], data[i * 3 + 1], data[i * 3 + 2]];
        let next = [data[j * 3], data[j * 3 + 1], data[j * 3 + 2]];
        
        normal[0] += (curr[1] - next[1]) * (curr[2] + next[2]);
        normal[1] += (curr[2] - next[2]) * (curr[0] + next[0]);
        normal[2] += (curr[0] - next[0]) * (curr[1] + next[1]);
    }
    
    // Find the dominant axis (largest component of normal)
    let abs_x = normal[0].abs();
    let abs_y = normal[1].abs();
    let abs_z = normal[2].abs();
    
    let mut projected_2d = Vec::with_capacity(num_vertices * 2);
    
    // Project to 2D by dropping the dominant axis
    if abs_z >= abs_x && abs_z >= abs_y {
        // Drop Z, use X,Y
        for i in 0..num_vertices {
            projected_2d.push(data[i * 3]);     // X
            projected_2d.push(data[i * 3 + 1]); // Y
        }
    } else if abs_y >= abs_x {
        // Drop Y, use X,Z
        for i in 0..num_vertices {
            projected_2d.push(data[i * 3]);     // X
            projected_2d.push(data[i * 3 + 2]); // Z
        }
    } else {
        // Drop X, use Y,Z
        for i in 0..num_vertices {
            projected_2d.push(data[i * 3 + 1]); // Y
            projected_2d.push(data[i * 3 + 2]); // Z
        }
    }
    
    web_sys::console::log_1(&format!("[PROJECTION DEBUG] Normal: {:?}, dropped axis: {}", normal, 
        if abs_z >= abs_x && abs_z >= abs_y { "Z" } else if abs_y >= abs_x { "Y" } else { "X" }).into());
    web_sys::console::log_1(&format!("[PROJECTION DEBUG] 2D projection: {:?}", projected_2d).into());
    
    // Verify the projected polygon has proper area
    let mut proj_area = 0.0;
    for i in 0..num_vertices {
        let j = (i + 1) % num_vertices;
        proj_area += (projected_2d[j * 2] - projected_2d[i * 2]) * (projected_2d[j * 2 + 1] + projected_2d[i * 2 + 1]);
    }
    proj_area *= 0.5;
    web_sys::console::log_1(&format!("[PROJECTION DEBUG] Projected polygon area: {}", proj_area).into());
    
    (projected_2d, true)
}

/// Main triangulation function that supports holes
pub fn earcut_triangulate(data: &[f64], hole_indices: Option<&[usize]>, dim: usize) -> Vec<usize> {
    web_sys::console::log_1(&format!("[EARCUT DEBUG] Starting earcut with data.len()={}, dim={}", data.len(), dim).into());
    web_sys::console::log_1(&format!("[EARCUT DEBUG] Data array: {:?}", data).into());
    
    // Project 3D to 2D if needed
    let (projected_data, was_projected) = if dim == 3 {
        project_3d_to_2d(data, dim)
    } else {
        (data.to_vec(), false)
    };
    
    let working_dim = if was_projected { 2 } else { dim };
    web_sys::console::log_1(&format!("[EARCUT DEBUG] Using {} coordinates, was_projected={}", working_dim, was_projected).into());
    
    let has_holes = hole_indices.is_some() && !hole_indices.unwrap().is_empty();
    let outer_len = if has_holes { 
        hole_indices.unwrap()[0] * working_dim 
    } else { 
        projected_data.len() 
    };
    
    web_sys::console::log_1(&format!("[EARCUT DEBUG] outer_len={}, has_holes={}", outer_len, has_holes).into());
    
    // Create outer ring using projected data
    let mut nodes = Vec::new();
    let outer_node = linked_list(&projected_data, 0, outer_len, working_dim, true, &mut nodes);
    let mut triangles = Vec::new();
    
    web_sys::console::log_1(&format!("[EARCUT DEBUG] linked_list returned: {:?}, nodes.len()={}", outer_node, nodes.len()).into());
    
    if projected_data.is_empty() || outer_len <= working_dim * 2 {
        web_sys::console::log_1(&format!("[EARCUT DEBUG] Early return: data.is_empty()={}, outer_len <= dim*2={}", projected_data.is_empty(), outer_len <= working_dim * 2).into());
        return triangles;
    }
    
    let mut min_x = 0.0;
    let mut min_y = 0.0;
    let mut max_x = 0.0;
    let mut max_y = 0.0;
    let mut inv_size = 0.0;
    
    if projected_data.len() > 80 * working_dim {
        min_x = projected_data[0];
        max_x = projected_data[0];
        min_y = projected_data[1];
        max_y = projected_data[1];
        
        // Calculate polygon bbox for z-order curve hash
        for i in (working_dim..outer_len).step_by(working_dim) {
            let x = projected_data[i];
            let y = projected_data[i + 1];
            if x < min_x { min_x = x; }
            if y < min_y { min_y = y; }
            if x > max_x { max_x = x; }
            if y > max_y { max_y = y; }
        }
        
        // Calculate coordinate space transformation
        inv_size = (max_x - min_x).max(max_y - min_y);
        inv_size = if inv_size != 0.0 { 32767.0 / inv_size } else { 0.0 };
    }
    
    if let Some(outer) = outer_node {
        // Link holes to outer ring
        let mut outer_idx = outer;
        if has_holes {
            if let Some(holes) = hole_indices {
                if let Some(new_outer) = eliminate_holes(&projected_data, holes, outer, working_dim, &mut nodes) {
                    outer_idx = new_outer;
                }
            }
        }
        
        // Run earcut algorithm
        if projected_data.len() > 80 * working_dim {
            web_sys::console::log_1(&"[EARCUT DEBUG] Using z-order curve optimization".into());
            earcut_linked(&mut nodes, outer_idx, &mut triangles, min_x, min_y, inv_size, true);
        } else {
            web_sys::console::log_1(&"[EARCUT DEBUG] Using simple triangulation".into());
            earcut_linked(&mut nodes, outer_idx, &mut triangles, 0.0, 0.0, 0.0, false);
        }
        
        web_sys::console::log_1(&format!("[EARCUT DEBUG] earcut_linked completed, triangles.len()={}", triangles.len()).into());
    } else {
        web_sys::console::log_1(&"[EARCUT DEBUG] outer_node is None - no triangulation possible".into());
    }
    
    web_sys::console::log_1(&format!("[EARCUT DEBUG] Final result: {} triangles", triangles.len()).into());
    triangles
}

/// Create a doubly linked list from polygon points in the specified order
fn linked_list(data: &[f64], start: usize, end: usize, dim: usize, clockwise: bool, nodes: &mut Vec<Node>) -> Option<usize> {
    web_sys::console::log_1(&format!("[LINKED_LIST DEBUG] start={}, end={}, dim={}, clockwise={}", start, end, dim, clockwise).into());
    
    let start_idx = nodes.len();
    
    let area = signed_area(data, start, end, dim);
    web_sys::console::log_1(&format!("[LINKED_LIST DEBUG] signed_area={}, condition: clockwise == (area > 0) = {} == {}", area, clockwise, area > 0.0).into());
    
    if clockwise == (signed_area(data, start, end, dim) > 0.0) {
        for i in (start..end).step_by(dim) {
            let node = Node::new(i / dim, data[i], data[i + 1], if dim > 2 { data[i + 2] } else { 0.0 });
            web_sys::console::log_1(&format!("[LINKED_LIST DEBUG] Adding node: x={}, y={}, z={}", data[i], data[i + 1], if dim > 2 { data[i + 2] } else { 0.0 }).into());
            nodes.push(node);
        }
    } else {
        for i in ((start..end).step_by(dim)).rev() {
            let node = Node::new(i / dim, data[i], data[i + 1], if dim > 2 { data[i + 2] } else { 0.0 });
            web_sys::console::log_1(&format!("[LINKED_LIST DEBUG] Adding node (reverse): x={}, y={}, z={}", data[i], data[i + 1], if dim > 2 { data[i + 2] } else { 0.0 }).into());
            nodes.push(node);
        }
    }
    
    let end_idx = nodes.len();
    web_sys::console::log_1(&format!("[LINKED_LIST DEBUG] start_idx={}, end_idx={}, nodes created: {}", start_idx, end_idx, end_idx - start_idx).into());
    
    if start_idx >= end_idx {
        web_sys::console::log_1(&"[LINKED_LIST DEBUG] Returning None - no nodes created".into());
        return None;
    }
    
    // Link the nodes in a doubly linked list
    for i in start_idx..end_idx {
        let local_i = i - start_idx;
        let len = end_idx - start_idx;
        let next_i = start_idx + (local_i + 1) % len;
        let prev_i = start_idx + if local_i == 0 { len - 1 } else { local_i - 1 };
        
        nodes[i].next = Some(next_i);
        nodes[i].prev = Some(prev_i);
    }
    
    Some(start_idx) // Return index of first node
}

/// Calculate signed area of a polygon
fn signed_area(data: &[f64], start: usize, end: usize, dim: usize) -> f64 {
    let mut sum = 0.0;
    let mut j = end - dim;
    
    for i in (start..end).step_by(dim) {
        sum += (data[j] - data[i]) * (data[i + 1] + data[j + 1]);
        j = i;
    }
    
    sum
}

/// Count active (non-removed) nodes
fn count_active_nodes(nodes: &[Node]) -> usize {
    nodes.iter().filter(|n| !n.removed).count()
}

/// Get next active (non-removed) node index
fn get_next_active(nodes: &[Node], mut idx: usize) -> Option<usize> {
    let start_idx = idx;
    loop {
        if let Some(next) = nodes[idx].next {
            idx = next;
            if idx < nodes.len() && !nodes[idx].removed {
                return Some(idx);
            }
            if idx == start_idx {
                break; // Avoid infinite loop
            }
        } else {
            break;
        }
    }
    None
}

/// Main ear slicing function
fn earcut_linked(nodes: &mut Vec<Node>, ear_idx: usize, triangles: &mut Vec<usize>, 
                min_x: f64, min_y: f64, inv_size: f64, use_hash: bool) {
    web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Starting with {} nodes, ear_idx={}", nodes.len(), ear_idx).into());
    
    if nodes.is_empty() {
        web_sys::console::log_1(&"[EARCUT_LINKED DEBUG] Empty nodes - returning".into());
        return;
    }
    
    // Initialize z-order curve hash if needed
    if use_hash {
        index_curve(nodes, min_x, min_y, inv_size);
    }
    
    let mut ear = ear_idx;
    let mut iterations = 0;
    let max_iterations = nodes.len() * nodes.len(); // Prevent infinite loops
    
    // Continue slicing ears until we have a triangle
    while count_active_nodes(nodes) > 3 && iterations < max_iterations {
        iterations += 1;
        let active_count = count_active_nodes(nodes);
        web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Iteration {}, active_nodes={}", iterations, active_count).into());
        
        let mut valid_ear_found = false;
        
        // Try to find and slice a valid ear
        let mut best_ear = None;
        let mut smallest_area = f64::INFINITY;
        
        // First pass: find the best (smallest valid) ear
        for _attempt in 0..active_count {
            // Check bounds and removed status
            if ear >= nodes.len() || nodes[ear].removed {
                web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Invalid ear index {} or removed node", ear).into());
                if let Some(next_active) = get_next_active(nodes, ear) {
                    ear = next_active;
                    continue;
                } else {
                    web_sys::console::log_1(&"[EARCUT_LINKED DEBUG] No active nodes found - breaking".into());
                    break;
                }
            }
            
            if is_ear(nodes, ear) {
                // Calculate triangle area for this ear
                let prev = match nodes[ear].prev {
                    Some(p) if p < nodes.len() && !nodes[p].removed => p,
                    _ => {
                        if let Some(next_active) = get_next_active(nodes, ear) {
                            ear = next_active;
                            continue;
                        } else {
                            break;
                        }
                    }
                };
                
                let next = match nodes[ear].next {
                    Some(n) if n < nodes.len() && !nodes[n].removed => n,
                    _ => {
                        if let Some(next_active) = get_next_active(nodes, ear) {
                            ear = next_active;
                            continue;
                        } else {
                            break;
                        }
                    }
                };
                
                let tri_area = area(&nodes[prev], &nodes[ear], &nodes[next]).abs();
                web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Found valid ear {} with area {}", ear, tri_area).into());
                
                if tri_area < smallest_area {
                    smallest_area = tri_area;
                    best_ear = Some(ear);
                }
            }
            
            // Move to next node safely
            if let Some(next_active) = get_next_active(nodes, ear) {
                ear = next_active;
            } else {
                web_sys::console::log_1(&"[EARCUT_LINKED DEBUG] No next active node found".into());
                break;
            }
        }
        
        // Cut the best ear if found
        if let Some(best_ear_idx) = best_ear {
            web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Cutting best ear {} with area {}", best_ear_idx, smallest_area).into());
            
            // Get next and prev with safety checks
            let next = match nodes[best_ear_idx].next {
                Some(n) if n < nodes.len() && !nodes[n].removed => n,
                _ => {
                    web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Invalid next node for best ear {}", best_ear_idx).into());
                    break;
                }
            };
            
            let prev = match nodes[best_ear_idx].prev {
                Some(p) if p < nodes.len() && !nodes[p].removed => p,
                _ => {
                    web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Invalid prev node for best ear {}", best_ear_idx).into());
                    break;
                }
            };
            
            // Validate vertex indices before adding triangle
            let prev_idx = nodes[prev].i;
            let ear_idx = nodes[best_ear_idx].i;
            let next_idx = nodes[next].i;
            
            // Find the maximum valid vertex index from the original vertices
            // Original vertices have the lowest vertex indices (before any Steiner points)
            let max_original_vertex_idx = nodes.iter()
                .filter(|n| !n.steiner)
                .map(|n| n.i)
                .max()
                .unwrap_or(0);
            
            if prev_idx <= max_original_vertex_idx && ear_idx <= max_original_vertex_idx && next_idx <= max_original_vertex_idx {
                triangles.push(prev_idx);
                triangles.push(ear_idx);
                triangles.push(next_idx);
                
                web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Added valid triangle: {}, {}, {} (coords: ({},{},{}) ({},{},{}) ({},{},{}))", 
                    prev_idx, ear_idx, next_idx,
                    nodes[prev].x, nodes[prev].y, nodes[prev].z,
                    nodes[best_ear_idx].x, nodes[best_ear_idx].y, nodes[best_ear_idx].z,
                    nodes[next].x, nodes[next].y, nodes[next].z).into());
            } else {
                web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] INVALID triangle indices: {}, {}, {} (max_original={})", 
                    prev_idx, ear_idx, next_idx, max_original_vertex_idx).into());
            }
            
            // Verify triangle winding
            let tri_area = area(&nodes[prev], &nodes[best_ear_idx], &nodes[next]);
            web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Triangle area: {}", tri_area).into());
            
            // Remove ear from linked list
            remove_node(nodes, best_ear_idx);
            
            // Move to next potential ear
            ear = next;
            valid_ear_found = true;
        }
        
        // If no ear found, for simple polygons we should always be able to find an ear
        // Bridge creation should only be used for polygons with holes
        if !valid_ear_found {
            web_sys::console::log_1(&"[EARCUT_LINKED DEBUG] No valid ear found".into());
            
            // For simple polygons, force triangulation of remaining vertices
            // This should not happen in a correct implementation, but provides fallback
            let active_count = count_active_nodes(nodes);
            if active_count >= 3 {
                web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Force triangulating {} remaining vertices", active_count).into());
                
                let mut active_indices: Vec<usize> = nodes.iter()
                    .enumerate()
                    .filter(|(_, n)| !n.removed)
                    .map(|(i, _)| i)
                    .collect();
                
                // Create triangles from remaining vertices (fan triangulation as fallback)
                if active_indices.len() >= 3 {
                    let center_idx = active_indices[0];
                    for i in 1..active_indices.len()-1 {
                        let idx0 = nodes[center_idx].i;
                        let idx1 = nodes[active_indices[i]].i;
                        let idx2 = nodes[active_indices[i+1]].i;
                        
                        let max_original_vertex_idx = nodes.iter()
                            .filter(|n| !n.steiner)
                            .map(|n| n.i)
                            .max()
                            .unwrap_or(0);
                        
                        if idx0 <= max_original_vertex_idx && idx1 <= max_original_vertex_idx && idx2 <= max_original_vertex_idx {
                            triangles.push(idx0);
                            triangles.push(idx1);
                            triangles.push(idx2);
                            web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Fallback triangle: {}, {}, {}", idx0, idx1, idx2).into());
                        }
                    }
                }
            }
            
            break; // Exit main loop
        }
    }
    
    web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Main loop ended. active_nodes={}, iterations={}", count_active_nodes(nodes), iterations).into());
    
    // Add the final triangle if exactly 3 active vertices remain
    let active_count = count_active_nodes(nodes);
    if active_count == 3 {
        web_sys::console::log_1(&"[EARCUT_LINKED DEBUG] Adding final triangle".into());
        let active_nodes: Vec<usize> = nodes.iter()
            .enumerate()
            .filter(|(_, n)| !n.removed)
            .map(|(i, _)| i)
            .collect();
            
        if active_nodes.len() == 3 {
            // Validate final triangle indices
            let idx0 = nodes[active_nodes[0]].i;
            let idx1 = nodes[active_nodes[1]].i;
            let idx2 = nodes[active_nodes[2]].i;
            
            let max_original_vertex_idx = nodes.iter()
                .filter(|n| !n.steiner)
                .map(|n| n.i)
                .max()
                .unwrap_or(0);
            
            if idx0 <= max_original_vertex_idx && idx1 <= max_original_vertex_idx && idx2 <= max_original_vertex_idx {
                triangles.push(idx0);
                triangles.push(idx1);
                triangles.push(idx2);
                web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Final valid triangle: {}, {}, {}", idx0, idx1, idx2).into());
            } else {
                web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Final triangle INVALID: {}, {}, {} (max_original={})", 
                    idx0, idx1, idx2, max_original_vertex_idx).into());
            }
        }
    } else {
        web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Final active_count={} - not creating final triangle", active_count).into());
    }
    
    web_sys::console::log_1(&format!("[EARCUT_LINKED DEBUG] Completed. triangles.len()={}", triangles.len()).into());
}

/// Validate if a triangle is acceptable (not too large, not overlapping)
fn is_valid_triangle(nodes: &[Node], prev_idx: usize, ear_idx: usize, next_idx: usize) -> bool {
    let a = &nodes[prev_idx];
    let b = &nodes[ear_idx];
    let c = &nodes[next_idx];
    
    // Check triangle area (should be reasonable, not too large)
    let tri_area = area(a, b, c).abs();
    
    // Calculate polygon bounding box for reference
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    
    for node in nodes.iter() {
        if !node.removed {
            min_x = min_x.min(node.x);
            max_x = max_x.max(node.x);
            min_y = min_y.min(node.y);
            max_y = max_y.max(node.y);
        }
    }
    
    let bbox_area = (max_x - min_x) * (max_y - min_y);
    
    // Triangle shouldn't be more than 1/3 of the bounding box area
    if tri_area > bbox_area * 0.33 {
        web_sys::console::log_1(&format!("[VALIDATION DEBUG] Triangle too large: area={}, bbox_area={}", tri_area, bbox_area).into());
        return false;
    }
    
    // Check if triangle edges intersect with any other polygon edges
    for i in 0..nodes.len() {
        if nodes[i].removed || i == prev_idx || i == ear_idx || i == next_idx {
            continue;
        }
        
        if let Some(next_i) = nodes[i].next {
            if next_i >= nodes.len() || nodes[next_i].removed || next_i == prev_idx || next_i == ear_idx || next_i == next_idx {
                continue;
            }
            
            // Check if any triangle edge intersects with this polygon edge
            if segments_intersect(a.x, a.y, b.x, b.y, nodes[i].x, nodes[i].y, nodes[next_i].x, nodes[next_i].y) ||
               segments_intersect(b.x, b.y, c.x, c.y, nodes[i].x, nodes[i].y, nodes[next_i].x, nodes[next_i].y) ||
               segments_intersect(c.x, c.y, a.x, a.y, nodes[i].x, nodes[i].y, nodes[next_i].x, nodes[next_i].y) {
                web_sys::console::log_1(&format!("[VALIDATION DEBUG] Triangle intersects polygon edge").into());
                return false;
            }
        }
    }
    
    true
}

/// Check if two line segments intersect
fn segments_intersect(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> bool {
    let denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
    if denom.abs() < 1e-10 {
        return false; // Parallel lines
    }
    
    let t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / denom;
    let u = -((x1 - x2) * (y1 - y3) - (y1 - y2) * (x1 - x3)) / denom;
    
    // Check if intersection point is within both segments
    t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0
}

/// Find two vertices that can form a valid bridge to split the polygon
fn find_bridge_vertices(nodes: &[Node], start_idx: usize) -> Option<(usize, usize)> {
    if start_idx >= nodes.len() || nodes[start_idx].removed {
        return None;
    }
    
    // Start from a reflex vertex (one that forms a "dent" in the polygon)
    let mut current = start_idx;
    let mut attempts = 0;
    
    while attempts < nodes.len() {
        attempts += 1;
        
        if current >= nodes.len() || nodes[current].removed {
            if let Some(next) = get_next_active(nodes, current) {
                current = next;
                continue;
            } else {
                break;
            }
        }
        
        // Check if this is a reflex vertex
        if is_reflex_vertex(nodes, current) {
            web_sys::console::log_1(&format!("[BRIDGE DEBUG] Found reflex vertex at {}", current).into());
            
            // Try to find a suitable vertex to bridge to
            if let Some(bridge_target) = find_bridge_target(nodes, current) {
                web_sys::console::log_1(&format!("[BRIDGE DEBUG] Found bridge target {} for reflex vertex {}", bridge_target, current).into());
                return Some((current, bridge_target));
            }
        }
        
        // Move to next vertex
        if let Some(next) = get_next_active(nodes, current) {
            current = next;
        } else {
            break;
        }
        
        if current == start_idx {
            break; // Full circle
        }
    }
    
    web_sys::console::log_1(&"[BRIDGE DEBUG] No suitable bridge found".into());
    None
}

/// Check if a vertex is reflex (forms an inward angle)
fn is_reflex_vertex(nodes: &[Node], idx: usize) -> bool {
    if idx >= nodes.len() || nodes[idx].removed {
        return false;
    }
    
    let curr = &nodes[idx];
    
    let prev_idx = match curr.prev {
        Some(p) if p < nodes.len() && !nodes[p].removed => p,
        _ => return false,
    };
    
    let next_idx = match curr.next {
        Some(n) if n < nodes.len() && !nodes[n].removed => n,
        _ => return false,
    };
    
    let prev = &nodes[prev_idx];
    let next = &nodes[next_idx];
    
    // Calculate the cross product to determine if the vertex is reflex
    let cross = (curr.x - prev.x) * (next.y - curr.y) - (curr.y - prev.y) * (next.x - curr.x);
    
    // For counter-clockwise winding, reflex vertices have positive cross product
    cross > 0.0
}

/// Find a suitable target vertex to bridge to from a reflex vertex
fn find_bridge_target(nodes: &[Node], reflex_idx: usize) -> Option<usize> {
    if reflex_idx >= nodes.len() || nodes[reflex_idx].removed {
        return None;
    }
    
    let reflex = &nodes[reflex_idx];
    let mut best_target = None;
    let mut best_distance = f64::INFINITY;
    
    // Look for vertices that can form a valid diagonal
    for i in 0..nodes.len() {
        if i == reflex_idx || nodes[i].removed {
            continue;
        }
        
        // Skip adjacent vertices
        if nodes[reflex_idx].prev == Some(i) || nodes[reflex_idx].next == Some(i) {
            continue;
        }
        
        let target = &nodes[i];
        
        // Check if the diagonal is inside the polygon and doesn't intersect edges
        if is_valid_bridge_diagonal(nodes, reflex_idx, i) {
            let distance = ((reflex.x - target.x).powi(2) + (reflex.y - target.y).powi(2)).sqrt();
            
            if distance < best_distance {
                best_distance = distance;
                best_target = Some(i);
            }
        }
    }
    
    best_target
}

/// Check if a diagonal between two vertices is valid for bridge creation
fn is_valid_bridge_diagonal(nodes: &[Node], a_idx: usize, b_idx: usize) -> bool {
    if a_idx >= nodes.len() || b_idx >= nodes.len() || 
       nodes[a_idx].removed || nodes[b_idx].removed {
        return false;
    }
    
    let a = &nodes[a_idx];
    let b = &nodes[b_idx];
    
    // Check if diagonal intersects with any polygon edge
    for i in 0..nodes.len() {
        if nodes[i].removed {
            continue;
        }
        
        if let Some(next_i) = nodes[i].next {
            if next_i >= nodes.len() || nodes[next_i].removed {
                continue;
            }
            
            // Skip if this edge involves our diagonal endpoints
            if i == a_idx || i == b_idx || next_i == a_idx || next_i == b_idx {
                continue;
            }
            
            // Check intersection
            if segments_intersect(a.x, a.y, b.x, b.y, 
                                nodes[i].x, nodes[i].y, nodes[next_i].x, nodes[next_i].y) {
                return false;
            }
        }
    }
    
    // Check if the diagonal is inside the polygon by checking midpoint
    let mid_x = (a.x + b.x) * 0.5;
    let mid_y = (a.y + b.y) * 0.5;
    
    // Use point-in-polygon test for the midpoint
    point_in_polygon(nodes, mid_x, mid_y)
}

/// Test if a point is inside the polygon
fn point_in_polygon(nodes: &[Node], px: f64, py: f64) -> bool {
    let mut inside = false;
    
    let mut j = 0;
    while j < nodes.len() && (nodes[j].removed || nodes[j].next.is_none()) {
        j += 1;
    }
    
    if j >= nodes.len() {
        return false;
    }
    
    let mut i = j;
    loop {
        if let Some(next_i) = nodes[i].next {
            if next_i < nodes.len() && !nodes[next_i].removed {
                let xi = nodes[i].x;
                let yi = nodes[i].y;
                let xj = nodes[next_i].x;
                let yj = nodes[next_i].y;
                
                if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
                    inside = !inside;
                }
                
                i = next_i;
                if i == j {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }
    
    inside
}

/// Split polygon using a bridge and triangulate the resulting parts
fn split_polygon_with_bridge(nodes: &mut Vec<Node>, bridge_a: usize, bridge_b: usize, _triangles: &mut Vec<usize>) -> bool {
    if bridge_a >= nodes.len() || bridge_b >= nodes.len() || 
       nodes[bridge_a].removed || nodes[bridge_b].removed {
        return false;
    }
    
    web_sys::console::log_1(&format!("[SPLIT DEBUG] Splitting polygon with bridge {}-{}", bridge_a, bridge_b).into());
    
    // Create two new nodes for the bridge endpoints
    let node_a = nodes[bridge_a].clone();
    let node_b = nodes[bridge_b].clone();
    
    // Create bridge by duplicating the endpoints
    let new_a_idx = nodes.len();
    let new_b_idx = nodes.len() + 1;
    
    let mut new_a = node_a.clone();
    new_a.removed = false;
    
    let mut new_b = node_b.clone();
    new_b.removed = false;
    
    // Set up the bridge connections
    // First polygon: bridge_a -> ... -> bridge_b -> new_a -> bridge_a
    new_a.prev = Some(bridge_b);
    new_a.next = nodes[bridge_a].next;
    
    // Second polygon: new_b -> ... -> new_a -> bridge_b -> new_b  
    new_b.prev = Some(new_a_idx);
    new_b.next = nodes[bridge_b].next;
    
    // Update existing connections
    if let Some(next_a) = nodes[bridge_a].next {
        if next_a < nodes.len() && !nodes[next_a].removed {
            nodes[next_a].prev = Some(new_a_idx);
        }
    }
    
    if let Some(next_b) = nodes[bridge_b].next {
        if next_b < nodes.len() && !nodes[next_b].removed {
            nodes[next_b].prev = Some(new_b_idx);
        }
    }
    
    // Connect the bridge
    nodes[bridge_a].next = Some(new_b_idx);
    nodes[bridge_b].next = Some(new_a_idx);
    new_b.prev = Some(bridge_a);
    new_a.prev = Some(bridge_b);
    
    nodes.push(new_a);
    nodes.push(new_b);
    
    web_sys::console::log_1(&format!("[SPLIT DEBUG] Created bridge nodes {} and {}", new_a_idx, new_b_idx).into());
    
    // Now try to triangulate the split polygons
    let active_count = count_active_nodes(nodes);
    web_sys::console::log_1(&format!("[SPLIT DEBUG] Active nodes after split: {}", active_count).into());
    
    true
}

/// Check if a vertex forms a valid ear
fn is_ear(nodes: &[Node], ear_idx: usize) -> bool {
    if ear_idx >= nodes.len() || nodes[ear_idx].removed {
        return false;
    }
    
    let ear = &nodes[ear_idx];
    
    let prev_idx = match ear.prev {
        Some(p) if p < nodes.len() && !nodes[p].removed => p,
        _ => return false,
    };
    
    let next_idx = match ear.next {
        Some(n) if n < nodes.len() && !nodes[n].removed => n,
        _ => return false,
    };
    
    let a = &nodes[prev_idx];
    let b = ear;
    let c = &nodes[next_idx];
    
    // Calculate triangle area to check orientation
    let triangle_area = area(a, b, c);
    web_sys::console::log_1(&format!("[IS_EAR DEBUG] Testing ear {}: triangle area = {}", ear_idx, triangle_area).into());
    
    // Check if the triangle is oriented correctly (convex)
    // For counter-clockwise winding, area should be negative
    if triangle_area >= 0.0 {
        web_sys::console::log_1(&format!("[IS_EAR DEBUG] Ear {} rejected: reflex vertex (area >= 0)", ear_idx).into());
        return false; // Reflex vertex
    }
    
    // Validate triangle before checking for interior points
    if !is_valid_triangle(nodes, prev_idx, ear_idx, next_idx) {
        web_sys::console::log_1(&format!("[IS_EAR DEBUG] Ear {} rejected: invalid triangle", ear_idx).into());
        return false;
    }
    
    // Check if any other vertex lies inside the triangle
    let mut p_idx = nodes[next_idx].next;
    let mut check_count = 0;
    while let Some(idx) = p_idx {
        check_count += 1;
        if check_count > nodes.len() {
            web_sys::console::log_1(&"[IS_EAR DEBUG] Breaking infinite loop in point checking".into());
            break;
        }
        
        if idx == prev_idx {
            break;
        }
        
        if idx >= nodes.len() || nodes[idx].removed {
            p_idx = nodes.get(idx).and_then(|n| n.next);
            continue;
        }
        
        let p = &nodes[idx];
        if point_in_triangle(a.x, a.y, b.x, b.y, c.x, c.y, p.x, p.y) {
            web_sys::console::log_1(&format!("[IS_EAR DEBUG] Ear {} rejected: point {} inside triangle", ear_idx, idx).into());
            return false;
        }
        
        p_idx = nodes[idx].next;
    }
    
    web_sys::console::log_1(&format!("[IS_EAR DEBUG] Ear {} accepted!", ear_idx).into());
    true
}

/// Calculate twice the signed area of triangle abc
fn area(a: &Node, b: &Node, c: &Node) -> f64 {
    (b.y - a.y) * (c.x - b.x) - (b.x - a.x) * (c.y - b.y)
}

/// Check if point p is inside triangle abc using barycentric coordinates
fn point_in_triangle(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, px: f64, py: f64) -> bool {
    // Use cross products to determine if point is on the same side of all edges
    let denom = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
    if denom.abs() < 1e-10 {
        return false; // Degenerate triangle
    }
    
    let a = ((by - cy) * (px - cx) + (cx - bx) * (py - cy)) / denom;
    let b = ((cy - ay) * (px - cx) + (ax - cx) * (py - cy)) / denom;
    let c = 1.0 - a - b;
    
    // Point is inside if all barycentric coordinates are non-negative
    a >= 0.0 && b >= 0.0 && c >= 0.0
}

/// Remove a node from the doubly linked list by marking it as removed
fn remove_node(nodes: &mut Vec<Node>, idx: usize) {
    if idx >= nodes.len() || nodes[idx].removed {
        web_sys::console::log_1(&format!("[REMOVE_NODE DEBUG] Invalid removal attempt: idx={}, len={}, already_removed={}", 
            idx, nodes.len(), idx < nodes.len() && nodes[idx].removed).into());
        return;
    }
    
    web_sys::console::log_1(&format!("[REMOVE_NODE DEBUG] Removing node {}", idx).into());
    
    let prev_idx = nodes[idx].prev;
    let next_idx = nodes[idx].next;
    
    // Update previous node's next pointer
    if let Some(prev) = prev_idx {
        if prev < nodes.len() && !nodes[prev].removed {
            nodes[prev].next = next_idx;
        }
    }
    
    // Update next node's previous pointer
    if let Some(next) = next_idx {
        if next < nodes.len() && !nodes[next].removed {
            nodes[next].prev = prev_idx;
        }
    }
    
    // Mark node as removed instead of deleting it
    nodes[idx].removed = true;
    
    web_sys::console::log_1(&format!("[REMOVE_NODE DEBUG] Node {} marked as removed", idx).into());
}

/// Get previous node index
fn get_prev_idx(nodes: &[Node], idx: usize) -> usize {
    nodes[idx].prev.unwrap_or(idx)
}

/// Get next node index
fn get_next_idx(nodes: &[Node], idx: usize) -> usize {
    nodes[idx].next.unwrap_or(idx)
}

/// Index the polygon using z-order curve hash for performance
fn index_curve(nodes: &mut [Node], min_x: f64, min_y: f64, inv_size: f64) {
    for node in nodes.iter_mut() {
        node.z_order = z_order(
            ((node.x - min_x) * inv_size) as u32,
            ((node.y - min_y) * inv_size) as u32
        );
    }
}

/// Calculate z-order curve value for a point
fn z_order(x: u32, y: u32) -> u32 {
    let mut x = x;
    let mut y = y;
    
    x = (x | (x << 8)) & 0x00FF00FF;
    x = (x | (x << 4)) & 0x0F0F0F0F;
    x = (x | (x << 2)) & 0x33333333;
    x = (x | (x << 1)) & 0x55555555;
    
    y = (y | (y << 8)) & 0x00FF00FF;
    y = (y | (y << 4)) & 0x0F0F0F0F;
    y = (y | (y << 2)) & 0x33333333;
    y = (y | (y << 1)) & 0x55555555;
    
    x | (y << 1)
}

/// Find a valid diagonal to split the polygon
fn find_split_vertex(nodes: &[Node], start_idx: usize) -> Option<usize> {
    let mut p_idx = nodes[start_idx].next;
    
    while let Some(idx) = p_idx {
        if idx == start_idx {
            break;
        }
        
        if is_valid_diagonal(nodes, start_idx, idx) {
            return Some(idx);
        }
        
        p_idx = nodes[idx].next;
    }
    
    None
}

/// Check if diagonal between two vertices is valid
fn is_valid_diagonal(nodes: &[Node], a_idx: usize, b_idx: usize) -> bool {
    let a = &nodes[a_idx];
    let b = &nodes[b_idx];
    
    // Check if diagonal is locally inside the polygon
    area(&nodes[get_prev_idx(nodes, a_idx)], a, b) >= 0.0 &&
    area(a, &nodes[get_next_idx(nodes, a_idx)], b) >= 0.0 &&
    area(&nodes[get_prev_idx(nodes, b_idx)], b, a) >= 0.0 &&
    area(b, &nodes[get_next_idx(nodes, b_idx)], a) >= 0.0
}

/// Split polygon by connecting two vertices
fn split_polygon(nodes: &mut Vec<Node>, a_idx: usize, b_idx: usize) {
    // This is a simplified version - a full implementation would
    // create two separate polygons and triangulate them independently
    // For now, we'll just mark the connection
    
    // Create Steiner points to connect the vertices
    let a = nodes[a_idx].clone();
    let b = nodes[b_idx].clone();
    
    // Insert new nodes to create the split
    let steiner_a = Node {
        i: a.i,
        x: a.x,
        y: a.y,
        z: a.z,
        prev: Some(a_idx),
        next: Some(b_idx),
        z_order: a.z_order,
        prev_z: None,
        next_z: None,
        steiner: true,
        removed: false,
    };
    
    let steiner_b = Node {
        i: b.i,
        x: b.x,
        y: b.y,
        z: b.z,
        prev: Some(b_idx),
        next: Some(a_idx),
        z_order: b.z_order,
        prev_z: None,
        next_z: None,
        steiner: true,
        removed: false,
    };
    
    nodes.push(steiner_a);
    nodes.push(steiner_b);
}

/// Eliminate holes by connecting them to the outer ring
fn eliminate_holes(data: &[f64], hole_indices: &[usize], outer_node: usize, 
                  dim: usize, nodes: &mut Vec<Node>) -> Option<usize> {
    let mut queue = Vec::new();
    
    for i in 0..hole_indices.len() {
        let start = hole_indices[i] * dim;
        let end = if i < hole_indices.len() - 1 {
            hole_indices[i + 1] * dim
        } else {
            data.len()
        };
        
        if let Some(list) = linked_list(data, start, end, dim, false, nodes) {
            if list == get_next_idx(nodes, list) {
                nodes[list].steiner = true;
            }
            queue.push(get_leftmost(nodes, list));
        }
    }
    
    queue.sort_by(|a, b| nodes[*a].x.partial_cmp(&nodes[*b].x).unwrap());
    
    // Process holes from left to right
    let mut outer = outer_node;
    for &hole_node in &queue {
        if let Some(bridge) = find_hole_bridge(nodes, hole_node, outer) {
            outer = split_polygon_at_bridge(nodes, hole_node, bridge);
        }
    }
    
    Some(outer)
}

/// Find the leftmost point in a polygon ring
fn get_leftmost(nodes: &[Node], start_idx: usize) -> usize {
    let mut p_idx = start_idx;
    let mut leftmost = start_idx;
    
    loop {
        if nodes[p_idx].x < nodes[leftmost].x {
            leftmost = p_idx;
        }
        p_idx = nodes[p_idx].next.unwrap();
        if p_idx == start_idx {
            break;
        }
    }
    
    leftmost
}

/// Find a bridge connecting hole and outer ring
fn find_hole_bridge(nodes: &[Node], hole_idx: usize, outer_idx: usize) -> Option<usize> {
    let hole = &nodes[hole_idx];
    let mut p_idx = outer_idx;
    let hx = hole.x;
    let hy = hole.y;
    let mut qx = f64::NEG_INFINITY;
    let mut m: Option<usize> = None;
    
    // Find a segment intersected by a ray from the hole's leftmost point to the left
    loop {
        let p = &nodes[p_idx];
        let next_idx = nodes[p_idx].next.unwrap();
        let next = &nodes[next_idx];
        
        if hy <= p.y && hy >= next.y && next.y != p.y {
            let x = p.x + (hy - p.y) / (next.y - p.y) * (next.x - p.x);
            if x <= hx && x > qx {
                qx = x;
                if x == hx {
                    if hy == p.y {
                        return Some(p_idx);
                    }
                    if hy == next.y {
                        return Some(next_idx);
                    }
                }
                m = if p.x < next.x { Some(p_idx) } else { Some(next_idx) };
            }
        }
        
        p_idx = next_idx;
        if p_idx == outer_idx {
            break;
        }
    }
    
    m
}

/// Split polygon at bridge point
fn split_polygon_at_bridge(nodes: &mut Vec<Node>, hole_idx: usize, bridge_idx: usize) -> usize {
    // Create connections between hole and outer ring
    let hole = nodes[hole_idx].clone();
    let bridge = nodes[bridge_idx].clone();
    
    // Insert bridge connections
    let new_hole_node = Node {
        i: hole.i,
        x: hole.x,
        y: hole.y,
        z: hole.z,
        prev: Some(bridge_idx),
        next: hole.next,
        z_order: hole.z_order,
        prev_z: None,
        next_z: None,
        steiner: false,
        removed: false,
    };
    
    let new_bridge_node = Node {
        i: bridge.i,
        x: bridge.x,
        y: bridge.y,
        z: bridge.z,
        prev: Some(hole_idx),
        next: bridge.next,
        z_order: bridge.z_order,
        prev_z: None,
        next_z: None,
        steiner: false,
        removed: false,
    };
    
    // Update connections
    if let Some(hole_next) = hole.next {
        nodes[hole_next].prev = Some(nodes.len());
    }
    if let Some(bridge_next) = bridge.next {
        nodes[bridge_next].prev = Some(nodes.len() + 1);
    }
    
    nodes[hole_idx].next = Some(bridge_idx);
    nodes[bridge_idx].prev = Some(hole_idx);
    
    nodes.push(new_hole_node);
    nodes.push(new_bridge_node);
    
    nodes.len() - 2 // Return new hole node index
}

/// Public interface for triangulating BaseGeometry with robust hole support
pub fn triangulate_with_holes(geom: &BaseGeometry) -> Vec<usize> {
    let vertices = geom.get_vertices();
    let holes = geom.get_holes();
    
    if vertices.is_empty() {
        return Vec::new();
    }
    
    // Convert to flat coordinate array
    let mut data = Vec::new();
    for vertex in &vertices {
        data.push(vertex.x);
        data.push(vertex.y);
        data.push(vertex.z);
    }
    
    // Convert holes to hole indices
    let mut hole_indices = Vec::new();
    let mut current_index = vertices.len();
    
    for hole in &holes {
        hole_indices.push(current_index);
        for vertex in hole {
            data.push(vertex.x);
            data.push(vertex.y);
            data.push(vertex.z);
        }
        current_index += hole.len();
    }
    
    // Perform triangulation
    if hole_indices.is_empty() {
        earcut_triangulate(&data, None, 3)
    } else {
        earcut_triangulate(&data, Some(&hole_indices), 3)
    }
}

// =======================
// WASM BINDINGS AND TEST FUNCTIONS
// =======================

#[wasm_bindgen]
pub fn test_robust_triangulation() -> String {
    // Test 1: Simple triangle
    let mut simple_geom = BaseGeometry::new("test".to_string());
    simple_geom.add_vertices(vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.5, 1.0, 0.0),
    ]);
    
    let simple_result = triangulate_with_holes(&simple_geom);
    
    // Test 2: Square with hole
    let mut square_with_hole = BaseGeometry::new("square_hole".to_string());
    
    // Outer square
    square_with_hole.add_vertices(vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(4.0, 0.0, 0.0),
        Vector3::new(4.0, 4.0, 0.0),
        Vector3::new(0.0, 4.0, 0.0),
    ]);
    
    // Inner hole (square)
    square_with_hole.add_holes(vec![
        Vector3::new(1.0, 1.0, 0.0),
        Vector3::new(3.0, 1.0, 0.0),
        Vector3::new(3.0, 3.0, 0.0),
        Vector3::new(1.0, 3.0, 0.0),
    ]);
    
    let hole_result = triangulate_with_holes(&square_with_hole);
    
    format!(
        "Robust triangulation tests completed.\nSimple triangle: {} triangles\nSquare with hole: {} triangles",
        simple_result.len() / 3,
        hole_result.len() / 3
    )
}

/// Improved version of the original ear clipping that's more robust
pub fn triangulate_polygon_robust(vertices: Vec<Vector3>) -> Vec<Vec<u32>> {
    if vertices.len() < 3 {
        return Vec::new();
    }
    
    // Convert to the format expected by earcut
    let mut data = Vec::new();
    for vertex in &vertices {
        data.push(vertex.x);
        data.push(vertex.y);
        data.push(vertex.z);
    }
    
    let indices = earcut_triangulate(&data, None, 3);
    
    // Convert back to the original format
    let mut triangles = Vec::new();
    for chunk in indices.chunks(3) {
        if chunk.len() == 3 {
            triangles.push(vec![chunk[0] as u32, chunk[1] as u32, chunk[2] as u32]);
        }
    }
    
    triangles
}

/// Enhanced triangulation function that supports holes using the same calling pattern
/// This replaces triangulate_polygon_by_face but adds robust hole support
pub fn triangulate_polygon_by_face_with_holes(face_vertices: Vec<Vector3>, holes: Vec<Vec<Vector3>>) -> Vec<Vec<u32>> {
    web_sys::console::log_1(&format!("triangulate_polygon_by_face_with_holes called with {} vertices and {} holes", face_vertices.len(), holes.len()).into());
    
    if face_vertices.len() < 3 {
        web_sys::console::log_1(&"Not enough vertices for triangulation".into());
        return Vec::new();
    }
    
    // Convert to flat coordinate array for earcut
    let mut data = Vec::new();
    for vertex in &face_vertices {
        data.push(vertex.x);
        data.push(vertex.y);
        data.push(vertex.z);
    }
    
    web_sys::console::log_1(&format!("Created data array with {} elements", data.len()).into());
    
    // Convert holes to hole indices if any exist
    let mut hole_indices = Vec::new();
    let mut current_index = face_vertices.len();
    
    for hole in &holes {
        if hole.len() >= 3 {
            hole_indices.push(current_index);
            for vertex in hole {
                data.push(vertex.x);
                data.push(vertex.y);
                data.push(vertex.z);
            }
            current_index += hole.len();
        }
    }
    
    web_sys::console::log_1(&format!("Final data array: {} elements, hole indices: {:?}", data.len(), hole_indices).into());
    
    // Perform robust triangulation
    let indices = if hole_indices.is_empty() {
        earcut_triangulate(&data, None, 3)
    } else {
        earcut_triangulate(&data, Some(&hole_indices), 3)
    };
    
    web_sys::console::log_1(&format!("Earcut returned {} indices", indices.len()).into());
    
    // Convert back to the original format
    let mut triangles = Vec::new();
    for chunk in indices.chunks(3) {
        if chunk.len() == 3 {
            triangles.push(vec![chunk[0] as u32, chunk[1] as u32, chunk[2] as u32]);
        }
    }
    
    web_sys::console::log_1(&format!("Final result: {} triangles", triangles.len()).into());
    
    triangles
}

/// Backward compatibility wrapper that uses the robust algorithm
/// This can be used as a drop-in replacement for the original triangulate_polygon_by_face
pub fn triangulate_polygon_by_face_robust(face_vertices: Vec<Vector3>) -> Vec<Vec<u32>> {
    web_sys::console::log_1(&format!("triangulate_polygon_by_face_robust called with {} vertices", face_vertices.len()).into());
    
    if face_vertices.len() < 3 {
        web_sys::console::log_1(&"Not enough vertices for triangulation".into());
        return Vec::new();
    }
    
    let result = triangulate_polygon_by_face_with_holes(face_vertices, Vec::new());
    web_sys::console::log_1(&format!("Triangulation result: {} triangles", result.len()).into());
    
    result
}
