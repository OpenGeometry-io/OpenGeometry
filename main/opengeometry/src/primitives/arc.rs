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

use crate::brep::{Edge, Face, Brep, Vertex};
use crate::geometry::path::Path;
use crate::utility::bgeometry::BufferGeometry;
use openmaths::{Vector3, Matrix4};
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

impl Path for OGArc {
    fn get_points(&self) -> Vec<Vector3> {
        // Sample points along the arc. Here we use the number of segments for sampling.
        let mut points = Vec::new();
        let segments = self.segments.max(8); // Ensure minimum segments for quality
        
        for i in 0..=segments {
            let angle = self.start_angle + (self.end_angle - self.start_angle) * (i as f64 / segments as f64);
            let x = self.center.x + self.radius * angle.cos();
            let y = self.center.y;
            let z = self.center.z + self.radius * angle.sin();
            points.push(Vector3::new(x, y, z));
        }
        points
    }

    fn get_frames(&self) -> Vec<Matrix4> {
        let mut frames = Vec::new();
        let segments = self.segments.max(8); // Ensure minimum segments for quality
        
        for i in 0..=segments {
            let angle = self.start_angle + (self.end_angle - self.start_angle) * (i as f64 / segments as f64);
            let x = self.center.x + self.radius * angle.cos();
            let y = self.center.y;
            let z = self.center.z + self.radius * angle.sin();
            let position = Vector3::new(x, y, z);
            
            // Calculate tangent vector (perpendicular to radius in XZ plane)
            let radius_x = position.x - self.center.x;
            let radius_z = position.z - self.center.z;
            let tangent = Vector3::new(-radius_z, 0.0, radius_x).normalize();
            
            // Up vector for arc (assuming arc is on XY plane or parallel to it)
            let up = Vector3::new(0.0, 1.0, 0.0);
            
            // Right vector
            let right = tangent.cross(&up).normalize();

            // Create transformation matrix
            let matrix = Matrix4::set(
                right.x, up.x, tangent.x, position.x,
                right.y, up.y, tangent.y, position.y,
                right.z, up.z, tangent.z, position.z,
                0.0, 0.0, 0.0, 1.0,
            );

            frames.push(matrix);
        }
        frames
    }
}