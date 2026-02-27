/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Arc Primitive for OpenGeometry.
 * 
 * An Arc is a segment of a circle defined by a center, radius, start angle, end angle, and number of segments.
 * It can be used to create circular arcs in 3D space.
 * Created with a center, radius, start angle, end angle, and number of segments.
 **/

 // TODO: What if we create the Circle using the Formula for Angles.

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use crate::brep::{Brep, Vertex};
use crate::drawing::{Path2D, Vec2};
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGArc {
  id: String,
  center: Vector3,
  radius: f64,
  start_angle: f64,
  end_angle: f64,
  segments: u32,
  brep: Brep
}

#[wasm_bindgen]
impl OGArc {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGArc {

    let internal_id = Uuid::new_v4();

    OGArc {
      id,
      center: Vector3::new(0.0, 0.0, 0.0),
      radius: 1.0,
      start_angle: 0.0,
      end_angle: 2.0 * std::f64::consts::PI,
      segments: 32,
      brep: Brep::new(internal_id),
    }
  }

  #[wasm_bindgen]
  pub fn set_config(&mut self, center: Vector3, radius: f64, start_angle: f64, end_angle: f64, segments: u32) {
    self.center = center;
    self.radius = radius;
    self.start_angle = start_angle;
    self.end_angle = end_angle;
    self.segments = segments;
  }

  #[wasm_bindgen]
  pub fn generate_geometry(&mut self) {
    self.dispose_points();

    let mut angle = self.start_angle;
    let angle_diff = (self.end_angle - self.start_angle) / self.segments as f64;

    for _ in 0..self.segments + 1 {
      let x = self.center.x + self.radius * angle.cos();
      let y = self.center.y;
      let z = self.center.z + self.radius * angle.sin();
      self.brep.vertices.push(Vertex::new(self.brep.get_vertex_count() as u32, Vector3::new(x, y, z)));
      angle += angle_diff;
    }
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
    let serialized = serde_json::to_string(&self.brep).unwrap();
    serialized
  }

  // TODO: For Line based primitives we are iterating just vertices
  // Figure out if it's benefical to create a edges and faces for Arc as well - Technically it's not needed
  #[wasm_bindgen]
  pub fn get_geometry_serialized(&mut self) -> String {
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
}

/// Pure Rust methods for drawing/export (not exposed to WASM)
impl OGArc {
  /// Convert the arc to a 2D path for export.
  /// Projects from 3D to 2D using the X-Z plane (ignores Y coordinate).
  /// The arc is tessellated into line segments based on the segments count.
  pub fn to_path2d(&self) -> Path2D {
    let mut path = Path2D::new();
    
    let vertices = &self.brep.vertices;
    if vertices.len() < 2 {
      return path;
    }
    
    // Convert consecutive vertices into line segments
    for i in 0..vertices.len() - 1 {
      let start = Vec2::new(vertices[i].position.x, vertices[i].position.z);
      let end = Vec2::new(vertices[i + 1].position.x, vertices[i + 1].position.z);
      path.add_line(start, end);
    }
    
    // Check if it's a full circle (start_angle to end_angle is 2π)
    let angle_range = (self.end_angle - self.start_angle).abs();
    if (angle_range - 2.0 * std::f64::consts::PI).abs() < 0.001 {
      path.closed = true;
    }
    
    path
  }
  
  /// Convert the arc to a 2D path with custom projection.
  /// 
  /// # Arguments
  /// * `x_axis` - Which 3D axis becomes 2D X: 0 = X, 1 = Y, 2 = Z
  /// * `y_axis` - Which 3D axis becomes 2D Y: 0 = X, 1 = Y, 2 = Z
  pub fn to_path2d_with_projection(&self, x_axis: u8, y_axis: u8) -> Path2D {
    let mut path = Path2D::new();
    
    let vertices = &self.brep.vertices;
    if vertices.len() < 2 {
      return path;
    }
    
    let get_axis = |p: &Vector3, axis: u8| -> f64 {
      match axis {
        0 => p.x,
        1 => p.y,
        2 => p.z,
        _ => p.x,
      }
    };
    
    for i in 0..vertices.len() - 1 {
      let start = Vec2::new(
        get_axis(&vertices[i].position, x_axis),
        get_axis(&vertices[i].position, y_axis)
      );
      let end = Vec2::new(
        get_axis(&vertices[i + 1].position, x_axis),
        get_axis(&vertices[i + 1].position, y_axis)
      );
      path.add_line(start, end);
    }
    
    let angle_range = (self.end_angle - self.start_angle).abs();
    if (angle_range - 2.0 * std::f64::consts::PI).abs() < 0.001 {
      path.closed = true;
    }
    
    path
  }
}