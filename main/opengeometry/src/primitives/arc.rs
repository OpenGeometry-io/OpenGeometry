/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Arc Primitive for OpenGeometry.
 * 
 * An Arc is a segment of a circle defined by a center, radius, start angle, end angle, and number of segments.
 * It can be used to create circular arcs in 3D space.
 * Created with a center, radius, start angle, end angle, and number of segments.
 **/

 // TODO: What if we create the Circle using the Formula for Angles.

#[cfg(feature="wasm")] use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use crate::brep::{Edge, Face, Brep, Vertex};
use crate::utility::bgeometry::BufferGeometry;
use openmaths::Vector3;
use uuid::Uuid;

#[cfg_attr(feature="wasm", wasm_bindgen)]
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

impl OGArc {
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  pub fn id(&self) -> String {
    self.id.clone()
  }

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

  pub fn set_config(&mut self, _start: Vector3, _end: Vector3, _radius: f64) {
    // TODO: Implement set_config method for Arc
    // self.start = start;
    // self.end = end;
    // self.radius = _radius;
  }

  pub fn generate_geometry(&mut self) {
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
  pub fn dispose_points(&mut self) {
    self.brep.clear();
  }

  // Destroy and Free memory
  pub fn destroy(&mut self) {
    self.brep.clear();
    self.id.clear();
  }

  pub fn get_brep_serialized(&self) -> String {
    let serialized = serde_json::to_string(&self.brep).unwrap();
    serialized
  }

  // TODO: For Line based primitives we are iterating just vertices
  // Figure out if it's benefical to create a edges and faces for Arc as well - Technically it's not needed
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

#[cfg(feature="wasm")]
#[wasm_bindgen]
impl OGArc {
  #[wasm_bindgen(constructor)]
  pub fn wasm_new(id: String) -> OGArc { OGArc::new(id) }
  
  #[wasm_bindgen(setter = "set_id")]
  pub fn wasm_set_id(&mut self, id: String) { OGArc::set_id(self, id); }
  
  #[wasm_bindgen(getter = "id")]
  pub fn wasm_id(&self) -> String { OGArc::id(self) }
  
  pub fn wasm_set_config(&mut self, start: Vector3, end: Vector3, radius: f64) { self.set_config(start, end, radius); }
  pub fn wasm_generate_geometry(&mut self) { self.generate_geometry(); }
  pub fn wasm_dispose_points(&mut self) { self.dispose_points(); }
  pub fn wasm_destroy(&mut self) { self.destroy(); }
  pub fn wasm_get_brep_serialized(&self) -> String { self.get_brep_serialized() }
  pub fn wasm_get_geometry_serialized(&mut self) -> String { self.get_geometry_serialized() }
}