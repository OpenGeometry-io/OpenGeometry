/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Rectangle Primitive for OpenGeometry.
 * 
 * A Rectangle is defined by its center, width, and breadth.
 * It can be used to create rectangular shapes in 3D space.
 * Created with a center, width, and breadth.
 */
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use crate::brep::{Brep, Vertex};
use crate::utility::bgeometry::BufferGeometry;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGRectangle {
  id: String,
  center: Vector3,
  width: f64,
  breadth: f64,
  geometry: BufferGeometry,
  brep: Brep,
}

#[wasm_bindgen]
impl OGRectangle {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGRectangle {

    let internal_id = Uuid::new_v4();

    OGRectangle {
      id,
      center: Vector3::new(0.0, 0.0, 0.0),
      width: 1.0,
      breadth: 1.0,
      geometry: BufferGeometry::new(internal_id),
      brep: Brep::new(internal_id),
    }
  }

  // TODO: Implement clone method if needed
  // #[wasm_bindgen]
  // pub fn clone(&self) -> OGRectangle {
  //   OGRectangle {
  //     id: self.id.clone(),
  //     center: self.center.clone(),
  //     width: self.width,
  //     breadth: self.breadth,
  //     points: self.points.clone()
  //   }
  // }

  #[wasm_bindgen]
  pub fn set_config(&mut self, center: Vector3, width: f64, breadth: f64) {
    self.center = center;
    self.width = width;
    self.breadth = breadth;
  }

  #[wasm_bindgen]
  pub fn generate_geometry(&mut self) {
    self.brep.clear();

    let half_width = self.width / 2.0;
    let half_breadth = self.breadth / 2.0;
    let center = self.center.clone();

    let p1 = Vector3::new(-half_width, 0.0, -half_breadth).add(&center);
    let p2 = Vector3::new(half_width, 0.0, -half_breadth).add(&center);
    let p3 = Vector3::new(half_width, 0.0, half_breadth).add(&center);
    let p4 = Vector3::new(-half_width, 0.0, half_breadth).add(&center);

    self.brep.vertices.push(Vertex::new(0, p1));
    self.brep.vertices.push(Vertex::new(1, p2));
    self.brep.vertices.push(Vertex::new(2, p3));
    self.brep.vertices.push(Vertex::new(3, p4));
  }

  #[wasm_bindgen]
  pub fn get_brep_serialized(&self) -> String {
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

    // Last point is the first point to close the rectangle
    let first_vertex = self.brep.vertices.first().unwrap();
    vertex_buffer.push(first_vertex.position.x);
    vertex_buffer.push(first_vertex.position.y);
    vertex_buffer.push(first_vertex.position.z);

    let vertex_serialized = serde_json::to_string(&vertex_buffer).unwrap();
    vertex_serialized
  }

  // TODO: Implement properties and destroy methods
  // #[wasm_bindgen]
  // pub fn update_width(&mut self, width: f64) {
  //   self.destroy();
  //   self.width = width;
  // }

  // #[wasm_bindgen]
  // pub fn update_breadth(&mut self, breadth: f64) {
  //   self.destroy();
  //   self.breadth = breadth;
  // }

  // #[wasm_bindgen]
  // pub fn destroy(&mut self) {
  //   self.points.clear();
  // }
}