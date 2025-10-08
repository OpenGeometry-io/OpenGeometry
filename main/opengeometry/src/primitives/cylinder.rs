/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Cylinder Primitive for OpenGeometry.
 * 
 * Base created by default on XZ plane and etruded along Y axis.
 * 
 * There are two ways to create a cylinder:
 * 1. By creating a cylinder with a circle arc, create a Circle Poly Face and then extrude by a given height
 * 2. By creating a cylinder primitive with a given radius and height
 * 
 * This class is used to create a cylinder primitive(2) using radius and height.
 **/

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use crate::brep::{Brep, Vertex};
use crate::operations::extrude::extrude_brep_face;
use crate::operations::triangulate::triangulate_polygon_with_holes;
use openmaths::Vector3;
use uuid::Uuid;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGCylinder {
  id: String,
  center: Vector3,
  radius: f64,
  height: f64,
  angle: f64,
  segments: u32,
  brep: Brep,
}

#[wasm_bindgen]
impl OGCylinder {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGCylinder {

    let internal_id = Uuid::new_v4();

    OGCylinder {
      id: id.clone(),
      center: Vector3::new(0.0, 0.0, 0.0),
      radius: 1.0,
      height: 1.0,
      angle: 2.0 * std::f64::consts::PI,
      segments: 32,
      brep: Brep::new(internal_id),
    }
  }

  #[wasm_bindgen]
  pub fn set_config(&mut self, center: Vector3, radius: f64, height: f64, angle: f64, segments: u32) {
    self.center = center;
    self.radius = radius;
    self.height = height;
    self.angle = angle;
    self.segments = segments;

    self.generate_brep();
  }

  pub fn generate_brep(&mut self) {
    self.clean_geometry();
    self.generate_geometry();
  }

  pub fn clean_geometry(&mut self) {
    self.brep.clear();
  }

  #[wasm_bindgen]
  pub fn generate_geometry(&mut self) {
    let half_height = self.height / 2.0;
    let mut segment_count: u32 = self.segments;

    // Create Bottom BREP Circle
    // A good idea create BREP library for corresponding Primitives, e.g. like Below
    // let bottom_circle_brep = Brep::new_circle(
    //   self.center.x, 
    //   self.center.y - half_height, 
    //   self.center.z, 
    //   self.radius, 
    //   segment_count, 
    //   0.0, 
    //   2.0 * std::f64::consts::PI
    // );
    let mut bottom_circle_brep = Brep::new(Uuid::new_v4());

    // If the end angle makes a full circle then we don't need to add a center point
    if self.angle < 2.0 * std::f64::consts::PI {
      // TODO: Not sure if I should push edges and faces when creating temporary BREP
      bottom_circle_brep.vertices.push(Vertex::new(
        bottom_circle_brep.get_vertex_count() as u32,
        Vector3::new(self.center.x, self.center.y - half_height, self.center.z)));
      segment_count += 1;
    }

    let mut start_angle: f64 = 0.0;
    let angle_step = self.angle / self.segments as f64;
    for _ in 0..segment_count {
      let x = self.center.x + self.radius * start_angle.cos();
      let y = self.center.y - half_height;
      let z = self.center.z + self.radius * start_angle.sin();
      
      bottom_circle_brep.vertices.push(Vertex::new(
        bottom_circle_brep.get_vertex_count() as u32,
        Vector3::new(x, y, z)));
      start_angle += angle_step;
    }

    // Extrude the points to create the top circle
    let brep_data = extrude_brep_face(bottom_circle_brep, self.height);
    self.brep = brep_data.clone();

  }

  #[wasm_bindgen]
  pub fn get_brep_serialized(&self) -> String {
    // Serialize the BREP geometry to JSON
    let serialized = serde_json::to_string(&self.brep).unwrap();
    serialized
  }

  #[wasm_bindgen]
  pub fn get_geometry_serialized(&mut self) -> String {
    let mut vertex_buffer: Vec<f64> = Vec::new();
    let faces = self.brep.faces.clone();

    for face in &faces {
      let (face_vertices, holes_vertices) = self.brep.get_vertices_and_holes_by_face_id(face.id);

      if face_vertices.len() < 3 {
        continue;
      }

      let triangles = triangulate_polygon_with_holes(&face_vertices, &holes_vertices);

      // Combine outer and hole vertices into a single list for easy lookup
      let all_vertices: Vec<Vector3> = face_vertices
        .into_iter()
        .chain(holes_vertices.into_iter().flatten())
        .collect();

      // Build the final vertex buffer for rendering
      for triangle in triangles {
        for vertex_index in triangle {
          // The indices from earcutr correspond to our combined `all_vertices` list
          let vertex = &all_vertices[vertex_index];
          vertex_buffer.push(vertex.x);
          vertex_buffer.push(vertex.y);
          vertex_buffer.push(vertex.z);
        }
      }
    }

    serde_json::to_string(&vertex_buffer).unwrap()
  }

  #[wasm_bindgen]
  pub fn get_outline_geometry_serialized(&mut self) -> String {
    let mut vertex_buffer: Vec<f64> = Vec::new();

    let edges = self.brep.edges.clone();
    for edge in edges {
      let start_index = edge.v1 as usize;
      let end_index = edge.v2 as usize;

      let start_vertex = self.brep.vertices[start_index].clone();
      let end_vertex = self.brep.vertices[end_index].clone();

      vertex_buffer.push(start_vertex.position.x);
      vertex_buffer.push(start_vertex.position.y);
      vertex_buffer.push(start_vertex.position.z);

      vertex_buffer.push(end_vertex.position.x);
      vertex_buffer.push(end_vertex.position.y);
      vertex_buffer.push(end_vertex.position.z);
    }

    let vertex_serialized = serde_json::to_string(&vertex_buffer).unwrap();
    vertex_serialized
  }
}