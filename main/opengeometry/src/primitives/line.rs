/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Line Primitive for OpenGeometry.
 * 
 * A Line is defined by two points.
 * This line would only have two points, else it becomes a polyline.
 * Created with two arbitrary points, start and end.
 */

use crate::brep::{Brep, Vertex};
use crate::drawing::{Path2D, Vec2};
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGLine {
  id: String,
  brep: Brep,
  start: Vector3,
  end: Vector3,
}

impl Drop for OGLine {
  fn drop(&mut self) {
    // TODO: Add dispose for Vector3 in OpenMaths
    // self.start.dispose();
    // self.end.dispose();
    self.brep.clear();
    self.id.clear();
  }
}

#[wasm_bindgen]
impl OGLine {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGLine {
    OGLine {
      id,
      start: Vector3::new(1.0, 0.0, 0.0),
      end: Vector3::new(-1.0, 0.0, 0.0),
      brep: Brep::new(Uuid::new_v4()),
    }
  }

  #[wasm_bindgen]
  pub fn set_config(&mut self, start: Vector3, end: Vector3) {
    self.brep.clear();
    self.start = start;
    self.end = end;
  }

  #[wasm_bindgen]
  pub fn generate_geometry(&mut self) {
    let start_vertex = Vertex::new(0, self.start);
    let end_vertex = Vertex::new(1, self.end);

    self.brep.vertices.push(start_vertex);
    self.brep.vertices.push(end_vertex);
  }

  // Dispose
  #[wasm_bindgen]
  pub fn dispose_points(&mut self) {
    self.brep.clear();
  }

  // Destroy and Free memory
  #[wasm_bindgen]
  pub fn destroy(&mut self) {
    self.brep.clear();
    self.id.clear();
  }

  #[wasm_bindgen]
  pub fn get_brep_serialized(&self) -> String {
    // Serialize the BREP geometry
    let serialized = serde_json::to_string(&self.brep).unwrap();
    serialized
  }

  #[wasm_bindgen]
  pub fn get_geometry_serialized(&self) -> String {
    let mut vertex_buffer: Vec<f64> = Vec::new();

    let vertices = self.brep.vertices.clone();
    for vertex in vertices {
      vertex_buffer.push(vertex.position.x);
      vertex_buffer.push(vertex.position.y);
      vertex_buffer.push(vertex.position.z); 
    }

    let vertex_serialized = serde_json::to_string(&vertex_buffer).unwrap();
    vertex_serialized
  }

  pub fn get_dxf_serialized(&self) -> String {
    // TODO: Implement DXF serialization for line
    String::new()
  }
}

/// Pure Rust methods for drawing/export (not exposed to WASM)
impl OGLine {
  /// Convert the line to a 2D path for export.
  /// Projects from 3D to 2D using the X-Z plane (ignores Y coordinate).
  pub fn to_path2d(&self) -> Path2D {
    let mut path = Path2D::new();
    
    let start_2d = Vec2::new(self.start.x, self.start.z);
    let end_2d = Vec2::new(self.end.x, self.end.z);
    
    path.add_line(start_2d, end_2d);
    path
  }
  
  /// Convert the line to a 2D path with custom projection.
  /// 
  /// # Arguments
  /// * `x_axis` - Which 3D axis becomes 2D X: 0 = X, 1 = Y, 2 = Z
  /// * `y_axis` - Which 3D axis becomes 2D Y: 0 = X, 1 = Y, 2 = Z
  pub fn to_path2d_with_projection(&self, x_axis: u8, y_axis: u8) -> Path2D {
    let mut path = Path2D::new();
    
    let get_axis = |p: &Vector3, axis: u8| -> f64 {
      match axis {
        0 => p.x,
        1 => p.y,
        2 => p.z,
        _ => p.x,
      }
    };
    
    let start_2d = Vec2::new(get_axis(&self.start, x_axis), get_axis(&self.start, y_axis));
    let end_2d = Vec2::new(get_axis(&self.end, x_axis), get_axis(&self.end, y_axis));
    
    path.add_line(start_2d, end_2d);
    path
  }
}