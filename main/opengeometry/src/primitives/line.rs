/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Line Primitive for OpenGeometry.
 * 
 * A Line is defined by two points.
 * This line would only have two points, else it becomes a polyline.
 * Created with two arbitrary points, start and end.
 */

use crate::brep::{Brep, Vertex};
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use openmaths::Vector3;
use uuid::Uuid;
use dxf::Drawing;
use dxf::entities::*;

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
    let mut drawing = Drawing::new();
    let added_entity_ref = drawing.add_entity(Entity::new(EntityType::Line(Line::default())));
    // `added_entity_ref` is a reference to the newly added entity
    let data = drawing.
  }
}