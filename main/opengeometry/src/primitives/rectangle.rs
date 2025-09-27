/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Rectangle Primitive for OpenGeometry.
 * 
 * A Rectangle is defined by its center, width, and breadth.
 * It can be used to create rectangular shapes in 3D space.
 * Created with a center, width, and breadth.
 */
#[cfg(feature="wasm")] use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use crate::brep::{Edge, Face, Brep, Vertex};
use crate::utility::bgeometry::BufferGeometry;
use openmaths::Vector3;
use uuid::Uuid;

#[cfg_attr(feature="wasm", wasm_bindgen)]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGRectangle {
  id: String,
  center: Vector3,
  width: f64,
  breadth: f64,
  geometry: BufferGeometry,
  brep: Brep,
}

impl OGRectangle {
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  pub fn id(&self) -> String {
    self.id.clone()
  }

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

  pub fn set_config(&mut self, center: Vector3, width: f64, breadth: f64) {
    self.center = center;
    self.width = width;
    self.breadth = breadth;
  }

  pub fn generate_geometry(&mut self) {
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

  pub fn get_brep_serialized(&self) -> String {
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

// Re-export wasm methods when wasm feature is enabled by wrapping in a shim impl block
#[cfg(feature="wasm")]
#[wasm_bindgen]
impl OGRectangle {
  #[wasm_bindgen(constructor)]
  pub fn wasm_new(id: String) -> OGRectangle { OGRectangle::new(id) }
  pub fn wasm_set_config(&mut self, center: Vector3, width: f64, breadth: f64) { self.set_config(center, width, breadth); }
  pub fn wasm_generate_geometry(&mut self) { self.generate_geometry(); }
  pub fn wasm_get_brep_serialized(&self) -> String { self.get_brep_serialized() }
  pub fn wasm_get_geometry_serialized(&self) -> String { self.get_geometry_serialized() }
}

// Mesh conversion
use crate::geometry::mesh::{ToMesh, MeshBuffers};

impl ToMesh for OGRectangle {
  fn to_mesh(&self) -> MeshBuffers {
    // Expect 4 vertices (rectangle plane). Build 2 triangles: (0,1,2) (0,2,3)
    if self.brep.vertices.len() < 4 { return MeshBuffers::empty(); }
    let normal = [0.0f32, 1.0f32, 0.0f32];
    let mut positions = Vec::with_capacity(4);
    let mut normals = Vec::with_capacity(4);
    for v in &self.brep.vertices { positions.push([v.position.x as f32, v.position.y as f32, v.position.z as f32]); normals.push(normal); }
    let indices = vec![0u32,1,2, 0,2,3];
    MeshBuffers { positions, normals, indices }
  }
}