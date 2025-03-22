use core::num;
use std::{collections::HashMap, hash::Hash};

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Vector3D {
  pub x: f64,
  pub y: f64,
  pub z: f64,
}

#[wasm_bindgen]
impl Vector3D {
  #[wasm_bindgen(constructor)]
  pub fn create(x: f64, y: f64, z: f64) -> Vector3D {
    Vector3D { x, y, z }
  }

  pub fn update(&mut self, x: f64, y: f64, z: f64) {
    self.x = x;
    self.y = y;
    self.z = z;
  }

  pub fn add(&self, other: &Vector3D) -> Vector3D {
    Vector3D {
      x: self.x + other.x,
      y: self.y + other.y,
      z: self.z + other.z
    }
  }

  pub fn subtract(&self, other: &Vector3D) -> Vector3D {
    Vector3D {
      x: self.x - other.x,
      y: self.y - other.y,
      z: self.z - other.z
    }
  }

  pub fn add_scalar(&self, scalar: f64) -> Vector3D {
    Vector3D {
      x: self.x + scalar,
      y: self.y + scalar,
      z: self.z + scalar
    }
  }

  pub fn subtract_scalar(&self, scalar: f64) -> Vector3D {
    Vector3D {
      x: self.x - scalar,
      y: self.y - scalar,
      z: self.z - scalar
    }
  }

  pub fn add_extrude_in_up(&self, height: f64, up_vector: Vector3D) -> Vector3D {
    Vector3D {
      x: self.x + up_vector.x * height,
      y: self.y + up_vector.y * height,
      z: self.z + up_vector.z * height
    }
  }

  pub fn cross(&self, other: &Vector3D) -> Vector3D {
    Vector3D {
      x: self.y * other.z - self.z * other.y,
      y: self.z * other.x - self.x * other.z,
      z: self.x * other.y - self.y * other.x
    }
  }

  pub fn dot(&self, other: &Vector3D) -> f64 {
    self.x * other.x + self.y * other.y + self.z * other.z
  }

  pub fn clone(&self) -> Vector3D {
    Vector3D {
      x: self.x,
      y: self.y,
      z: self.z
    }
  }
}

#[wasm_bindgen]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Matrix3D {
  pub m11: f64, pub m12: f64, pub m13: f64,
  pub m21: f64, pub m22: f64, pub m23: f64,
  pub m31: f64, pub m32: f64, pub m33: f64,
}

#[wasm_bindgen]
impl Matrix3D {
  #[wasm_bindgen(constructor)]
  pub fn set(
    m11: f64, m12: f64, m13: f64,
    m21: f64, m22: f64, m23: f64,
    m31: f64, m32: f64, m33: f64,
  ) -> Matrix3D {
    Matrix3D { m11, m12, m13, m21, m22, m23, m31, m32, m33 }
  }

  pub fn add(&self, other: &Matrix3D) -> Matrix3D {
    Matrix3D {
      m11: self.m11 + other.m11, m12: self.m12 + other.m12, m13: self.m13 + other.m13,
      m21: self.m21 + other.m21, m22: self.m22 + other.m22, m23: self.m23 + other.m23,
      m31: self.m31 + other.m31, m32: self.m32 + other.m32, m33: self.m33 + other.m33,
    }
  }

  pub fn subtract(&self, other: &Matrix3D) -> Matrix3D {
    Matrix3D {
      m11: self.m11 - other.m11, m12: self.m12 - other.m12, m13: self.m13 - other.m13,
      m21: self.m21 - other.m21, m22: self.m22 - other.m22, m23: self.m23 - other.m23,
      m31: self.m31 - other.m31, m32: self.m32 - other.m32, m33: self.m33 - other.m33,
    }
  }
}

#[wasm_bindgen]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct ColorRGB {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

#[wasm_bindgen]
impl ColorRGB {
  #[wasm_bindgen(constructor)]
  pub fn new(r: u8, g: u8, b: u8) -> ColorRGB {
    ColorRGB { r, g, b }
  }

  pub fn to_hex(&self) -> String {
    format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
  }
}

#[wasm_bindgen]
pub struct Color {
  hex: String
}

#[wasm_bindgen]
impl Color {
  #[wasm_bindgen(constructor)]
  pub fn new(hex: String) -> Color {
    Color { hex }
  }

  #[wasm_bindgen]
  pub fn to_rgba(&self) -> Result<ColorRGB, String> {
    let hex = self.hex.trim_start_matches('#');
    let len = hex.len();

    if len != 6 && len != 8 {
        return Err("Hex string must be in the format #RRGGBB or #RRGGBBAA".to_string());
    }

    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid red component")?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid green component")?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid blue component")?;

    Ok(ColorRGB { r, g, b })
  }
}

pub struct EdgeStructured {
  pub start: Vector3D,
  pub end: Vector3D,
  pub color: ColorRGB,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Geometry {
  pub vertices: Vec<Vector3D>,
  pub edges: Vec<Vec<u8>>,
  pub faces: Vec<Vec<u8>>,
}

impl Geometry {
  fn new() -> Geometry {
    Geometry {
      vertices: Vec::new(),
      edges: Vec::new(),
      faces: Vec::new(),
    }
  }

  pub fn get_geometry(&self) -> String {
    // serialize geometry
    let serialized = serde_json::to_string(&self).unwrap();
    serialized
  }
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct Mesh {
  pub position: Vector3D,
  pub position_matrix: Matrix3D,
  pub rotation: Vector3D,
  pub rotation_matrix: Matrix3D,
  pub scale: Vector3D,
  pub scale_matrix: Matrix3D,
  pub color: ColorRGB,
  buf_faces: Vec<Vector3D>,
  poligon_vertices: Vec<Vector3D>,
  geometry: Geometry,
  extrude_direction: Vector3D,
}

#[wasm_bindgen]
impl Mesh {
  pub fn new() -> Mesh {
    Mesh {
      position: Vector3D::create(0.0, 0.0, 0.0),
      position_matrix: Matrix3D::set(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
      rotation: Vector3D::create(0.0, 0.0, 0.0),
      rotation_matrix: Matrix3D::set(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
      scale: Vector3D::create(1.0, 1.0, 1.0),
      scale_matrix: Matrix3D::set(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
      color: ColorRGB { r: 0, g: 0, b: 0 },
      buf_faces: Vec::new(),
      poligon_vertices: Vec::new(),
      geometry: Geometry::new(),
      extrude_direction: Vector3D::create(0.0, 1.0, 0.0),
    }
  }

  pub fn copy_poligon_vertices(&mut self, vertices: Vec<Vector3D>) {
    self.poligon_vertices = vertices;
  }

  pub fn get_poligon_vertices(&self) -> Vec<Vector3D> {
    self.poligon_vertices.clone()
  }

  pub fn add_buf_face(&mut self, vertex: Vector3D) {
    self.buf_faces.push(vertex);
  }

  pub fn remove_buf_face(&mut self, index: usize) {
    if index < self.buf_faces.len() {
      self.buf_faces.remove(index);
    }
  }

  pub fn set_position(&mut self, position: Vector3D) {
    self.position = position;
  }

  pub fn get_position(&self) -> Vector3D {
    self.position
  }

  // TODO: Fix this iteratively
  pub fn set_extrude_height(&mut self, height: f64) {
    if self.poligon_vertices.len() > 2 {
      let mut buf_vertices = self.poligon_vertices.clone();
      let mut buf_edges: Vec<Vec<u8>> = Vec::new();
      let mut buf_faces: Vec<Vec<u8>> = Vec::new();

      let current_length = self.poligon_vertices.len();

      // Iterate the exisitng vertices and create edges
      for i in 0..self.poligon_vertices.len() {
        let edge = {
          vec![i as u8, ((i + 1) % self.poligon_vertices.len()) as u8]
        };
        buf_edges.push(edge);
      }

      // Creating First Polygon Faces 
      let mut face: Vec<u8> = Vec::new();
      for i in 0..self.poligon_vertices.len() {
        face.push(i as u8);
      }
      buf_faces.push(face);

      // Iterate over vertices and extrude in up direction
      for index in 0..self.poligon_vertices.len() {
        let new_vertex = self.poligon_vertices[index].clone().add_extrude_in_up(height, Vector3D::create(0.0, 1.0, 0.0));
        buf_vertices.push(new_vertex);

        let edge = {
          vec![index as u8, buf_vertices.len() as u8 - 1]
        };
        
        buf_edges.push(edge);
      }

      // Iterate over new vertices starting from the last index of the original vertices
      for i in current_length..buf_vertices.len() {
        if i < buf_vertices.len() - 1 {
          let edge = {
            vec![i as u8, (i + 1) as u8]
          };
          buf_edges.push(edge);
        } else {
          let edge = {
            vec![i as u8, (current_length) as u8]
          };
          buf_edges.push(edge);
        }
      }

      // Side Faces
      for i in 0..current_length {
        let next = (i + 1) % current_length;
        let face: Vec<u8> = vec![
          i as u8,
          next as u8,
          (next + current_length) as u8,
          i as u8 + current_length as u8,
        ];
        buf_faces.push(face);
      }

      // Bottom Face
      let mut face: Vec<u8> = Vec::new();
      for i in 0..current_length {
        face.push(i as u8 + current_length as u8);
      }
      buf_faces.push(face);
      
      let geometry = Geometry {
        vertices: buf_vertices,
        edges: buf_edges,
        faces: buf_faces,
      };
      
      self.geometry = geometry;
    }
  }

  // TODO: Fix this iteratively
  // pub fn update_extrude_height(&mut self, height: f64) {}

  pub fn get_geometry(&self) -> String {
    self.geometry.get_geometry()
  }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct Polygon {
  vertices: Vec<Vector3D>,
  pub position: Vector3D,
  pub extrude: bool,
}

#[wasm_bindgen]
impl Polygon {
  #[wasm_bindgen(constructor)]
  pub fn new() -> Polygon {
    Polygon {
      vertices: Vec::new(),
      position: Vector3D::create(0.0, 0.0, 0.0),
      extrude: false,
    }
  }

  pub fn add_vertex(&mut self, vertex: Vector3D) {
    self.vertices.push(vertex);
  }

  pub fn remove_vertex(&mut self, index: usize) {
    if index < self.vertices.len() {
      self.vertices.remove(index);
    }
  }

  pub fn update_vertex(&mut self, index: usize, vertex: Vector3D) {
    if index < self.vertices.len() {
      self.vertices[index] = vertex;
    }
  }

  // Instead of returning Vec<Vector3D>, expose controlled access
  pub fn get_vertex(&self, index: usize) -> Option<Vector3D> {
    self.vertices.get(index).copied()
  }

  pub fn vertex_count(&self) -> usize {
    self.vertices.len()
  }

  pub fn get_all_vertices(&self) -> Vec<Vector3D> {
    self.vertices.clone()
  }

  pub fn clear_vertices(&mut self) {
    self.vertices.clear();
  }

  pub fn set_position(&mut self, position: Vector3D) {
    self.position = position;
  }

  pub fn get_position(&self) -> Vector3D {
    self.position
  }

  pub fn set_extrude(&mut self, extrude: bool) -> Mesh {
    self.extrude = extrude;

    let mut mesh = Mesh::new();
    mesh.position = self.position;
    mesh.poligon_vertices = self.vertices.clone();
    mesh
  }

  pub fn earcut(&self) -> Vec<f64> {
    let mut triangles: Vec<f64> = Vec::new();

    if self.vertices.len() > 2 {
      let indices = tricut(self.vertices.clone());
      for index in indices {
        for i in index {
          let vertex = self.vertices[i as usize];
          triangles.push(vertex.x);
          triangles.push(vertex.y);
          triangles.push(vertex.z);
        }
      }
    }

    triangles
  }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct Triangle {
  pub a: Vector3D,
  pub b: Vector3D,
  pub c: Vector3D,
}

#[wasm_bindgen]
impl Triangle {
  #[wasm_bindgen(constructor)]
  pub fn new() -> Triangle {
    Triangle {
      a: Vector3D::create(0.0, 0.0, 0.0),
      b: Vector3D::create(0.0, 0.0, 0.0),
      c: Vector3D::create(0.0, 0.0, 0.0),
    }
  }

  pub fn set_vertices(&mut self, a: Vector3D, b: Vector3D, c: Vector3D) {
    self.a = a;
    self.b = b;
    self.c = c;
  }

  // pub fn get_area(&self) -> f64 {
  //   let normal = self.get_normal();
  //   let area = 0.5 * normal.magnitude();
  //   area
  // }

  pub fn get_all_vertices(&self) -> Vec<Vector3D> {
    vec![self.a, self.b, self.c]
  }

  pub fn is_point_in_triangle(&self, p : Vector3D) -> bool {
    let ab = self.b.clone().subtract(&self.a);
    let bc = self.c.clone().subtract(&self.b);
    let ca = self.a.clone().subtract(&self.c);

    let ap = p.clone().subtract(&self.a);
    let bp = p.clone().subtract(&self.b);
    let cp = p.clone().subtract(&self.c);

    let cross_abp = ab.clone().cross(&ap);
    let cross_bcp = bc.clone().cross(&bp);
    let cross_cap = ca.clone().cross(&cp);

    if (
        cross_abp.y > 0.0 &&
        cross_bcp.y > 0.0 &&
        cross_cap.y > 0.0
      ) || (
        cross_abp.y < 0.0 &&
        cross_bcp.y < 0.0 &&
        cross_cap.y < 0.0
      ) {
        return true;
      }
    false
  }
}

#[wasm_bindgen]
pub fn triangulate_mesh(mesh: Mesh) -> Vec<f64> {
  let raw_mesh = mesh.clone();
  let raw_geometry = raw_mesh.geometry.clone();
  let raw_vertices = raw_geometry.vertices.clone();

  let mut triangles_vertices: Vec<f64> = Vec::new();

  // use tricut to triangulate the mesh
  for face in raw_geometry.faces {
    let mut vertices: Vec<Vector3D> = Vec::new();
    for index in face {
      vertices.push(raw_vertices[index as usize]);
    }
  
    let mut tri_faces = tricut(vertices.clone());
    for face in tri_faces {
      for index in face {
        let vertex = vertices[index as usize];
        triangles_vertices.push(vertex.x);
        triangles_vertices.push(vertex.y);
        triangles_vertices.push(vertex.z);
      }
    }
  }

  triangles_vertices
}

#[wasm_bindgen]
pub fn triangulate(vertices: Vec<Vector3D>) -> Vec<Vector3D> {
  let mut triangles: Vec<Vector3D> = Vec::new();

  if vertices.len() > 2 {
    for i in 1..vertices.len() - 1 {
      triangles.push(vertices[0]);
      triangles.push(vertices[i]);
      triangles.push(vertices[i + 1]);
    }
  }

  triangles
}


pub fn ear_triangle_test(
  vertices: HashMap<u32, Vec<f64>>,
  a_index: u32,
  b_index: u32,
  c_index: u32,
) -> bool {
  let point_a = Vector3D::create(
      vertices[&(a_index)][0],
      vertices[&(a_index)][1],
      vertices[&(a_index)][2],
  );
  let point_b = Vector3D::create(
      vertices[&(b_index)][0],
      vertices[&(b_index)][1],
      vertices[&(b_index)][2],
  );
  let point_c = Vector3D::create(
      vertices[&(c_index)][0],
      vertices[&(c_index)][1],
      vertices[&(c_index)][2],
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



// #[wasm_bindgen]
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
              // Reverse the order here to change winding
              triangle_indices.push(vec![a, c, b]); // changed from vec![a, b, c] to vec![a, c, b]
              remaining_vertices.remove(i);
              break;
          }
      }
  }
  
  // Reverse the order for the last triangle as well
  triangle_indices.push(vec![
      remaining_vertices[0],
      remaining_vertices[2], // changed from [0, 1, 2] to [0, 2, 1]
      remaining_vertices[1],
  ]);

  // serde_json::to_string(&triangle_indices).unwrap()
  triangle_indices
}

#[wasm_bindgen]
pub fn get_tricut_vertices() -> String {
  let mut all_vertices: HashMap<u32, Vec<f64>> = HashMap::new();
    
  // Define vertices
  all_vertices.insert(0, vec![3.0, 0.0, 48.0]);
  all_vertices.insert(1, vec![52.0, 0.0, 8.0]);
  all_vertices.insert(2, vec![99.0, 0.0, 50.0]);
  all_vertices.insert(3, vec![138.0, 0.0, 25.0]);
  all_vertices.insert(4, vec![175.0, 0.0, 77.0]);
  all_vertices.insert(5, vec![131.0, 0.0, 72.0]);
  all_vertices.insert(6, vec![111.0, 0.0, 113.0]);
  all_vertices.insert(7, vec![72.0, 0.0, 43.0]);
  all_vertices.insert(8, vec![26.0, 0.0, 55.0]);
  all_vertices.insert(9, vec![29.0, 0.0, 100.0]);

  serde_json::to_string(&all_vertices).unwrap()
}
