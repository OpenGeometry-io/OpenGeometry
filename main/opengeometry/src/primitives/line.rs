/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Line Primitive for OpenGeometry.
 * 
 * A Line is defined by two points.
 * This line would only have two points, else it becomes a polyline.
 * Created with two arbitrary points, start and end.
 */

use crate::brep::{Edge, Face, Brep, Vertex};
#[cfg(feature="wasm")] use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use openmaths::Vector3;
use uuid::Uuid;

#[cfg_attr(feature="wasm", wasm_bindgen)]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGLine {
  id: String,
  brep: Brep,
  start: Vector3,
  end: Vector3,
}

impl OGLine {
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  pub fn id(&self) -> String {
    self.id.clone()
  }

  pub fn new(id: String) -> OGLine {

    let internal_id = Uuid::new_v4();

    OGLine {
      id,
      brep: Brep::new(internal_id),
      start: Vector3::new(1.0, 0.0, 0.0),
      end: Vector3::new(-1.0, 0.0, 0.0),
    }
  }

  pub fn set_config(&mut self, start: Vector3, end: Vector3) {
    self.brep.clear();
    self.start = start;
    self.end = end;
  }

  pub fn generate_geometry(&mut self) {
    // Create vertices for the start and end points
    let start_vertex = Vertex::new(0, self.start);
    let end_vertex = Vertex::new(1, self.end);

    self.brep.vertices.push(start_vertex);
    self.brep.vertices.push(end_vertex);
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
    // Serialize the BREP geometry
    let serialized = serde_json::to_string(&self.brep).unwrap();
    serialized
  }

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
}

#[cfg(feature="wasm")]
#[wasm_bindgen]
impl OGLine {
  #[wasm_bindgen(constructor)]
  pub fn wasm_new(id: String) -> OGLine { OGLine::new(id) }
  
  #[wasm_bindgen(setter = "set_id")]
  pub fn wasm_set_id(&mut self, id: String) { OGLine::set_id(self, id); }
  
  #[wasm_bindgen(getter = "id")]
  pub fn wasm_id(&self) -> String { OGLine::id(self) }
  
  pub fn wasm_set_config(&mut self, start: Vector3, end: Vector3) { self.set_config(start, end); }
  pub fn wasm_generate_geometry(&mut self) { self.generate_geometry(); }
  pub fn wasm_dispose_points(&mut self) { self.dispose_points(); }
  pub fn wasm_destroy(&mut self) { self.destroy(); }
  pub fn wasm_get_brep_serialized(&self) -> String { self.get_brep_serialized() }
  pub fn wasm_get_geometry_serialized(&self) -> String { self.get_geometry_serialized() }
}