use core::str;
use std::clone;

use crate::operations::extrude::{self, extrude_polygon_by_buffer_geometry};
use crate::operations::triangulate::triangulate_polygon_by_face;
use crate::operations::windingsort;
use crate::utility::geometry::{Geometry};
use openmaths::Vector3;

/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Cylinder primitive for OpenGeometry.
 * 
 * Base created by default on XZ plane and etruded along Y axis.
 * 
 * There are two ways to create a cylinder:
 * 1. By creating a cylinder with a circle arc, create a Circle Poly Face and then extrude by a given height
 * 2. By creating a cylinder primitive with a given radius and height
 * 
 * This class is used to create a cylinder primitive(2) using radius and height.
 *  */
use crate::geometry::basegeometry;
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGCylinderOld {
  id: String,
  center: Vector3,
  radius: f64,
  height: f64,
  angle: f64,
  segments: u32,
  geometry: basegeometry::BaseGeometry,
  buffer: Vec<f64>,
  brep: Geometry,
}

#[wasm_bindgen]
impl OGCylinderOld {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGCylinderOld {
    OGCylinderOld {
      id: id.clone(),
      center: Vector3::new(0.0, 0.0, 0.0),
      radius: 1.0,
      height: 1.0,
      angle: 2.0 * std::f64::consts::PI,
      segments: 32,
      geometry: basegeometry::BaseGeometry::new(id.clone()),
      buffer: Vec::new(),
      brep: Geometry::new(),
    }
  }

  #[wasm_bindgen]
  pub fn set_config(&mut self, center: Vector3, radius: f64, height: f64, angle: f64, segments: u32) {
    self.center = center;
    self.radius = radius;
    self.height = height;
    self.angle = angle;
    self.segments = segments;
  }

  #[wasm_bindgen]
  pub fn generate_geometry(&mut self) {
    let mut points: Vec<Vector3> = Vec::new();
    let mut normals: Vec<Vector3> = Vec::new();
    // let mut uvs: Vec<openmath::Vector2D> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();


    let half_height = self.height / 2.0;
    let mut actual_segments: u32 = self.segments;

    // If the end angle makes a full circle then we don't need to add a center point
    if self.angle < 2.0 * std::f64::consts::PI {
      // Add center point
      points.push(Vector3::new(self.center.x, self.center.y - half_height, self.center.z));
      actual_segments += 1;
    }

    let mut start_angle: f64 = 0.0;
    let angle_step = self.angle / self.segments as f64;
    for _ in 0..actual_segments {
      let x = self.center.x + self.radius * start_angle.cos();
      let y = self.center.y - half_height;
      let z = self.center.z + self.radius * start_angle.sin();
      points.push(Vector3::new(x, y, z));

      // // Indices for the top circle
      // if i < self.segments - 1 {
      //   indices.push(i);
      //   indices.push(i + 1);
      //   indices.push(self.segments);
      // } else {
      //   indices.push(i);
      //   indices.push(0);
      //   indices.push(self.segments);
      // }

      start_angle += angle_step;
    }
    
    // Side Faces Indices

    // let ccw_points = windingsort::ccw_test(points.clone());
    // self.geometry.add_vertices(ccw_points.clone());
    let mut clonedpoints = points.clone();
    clonedpoints.reverse();
    self.geometry.add_vertices(clonedpoints);
    self.geometry.add_indices(indices);
  }

  fn generate_brep(&mut self) -> Geometry {
    let extrude_data = extrude_polygon_by_buffer_geometry(self.geometry.clone(), self.height);
    self.brep = extrude_data.clone();
    extrude_data
  }

  #[wasm_bindgen]
  pub fn get_geometry(&mut self) -> String {
    let geometry = self.geometry.get_geometry();
    // geometry
    let extrude_data = self.generate_brep();
    
    let mut local_geometry = Vec::new();
    
    // let face = extrude_data.faces[0].clone();
    for face in extrude_data.faces.clone() {
      let mut face_vertices: Vec<Vector3> = Vec::new();
      for index in face.clone() {
        face_vertices.push(extrude_data.vertices[index as usize].clone());
      }

      let triangulated_face = triangulate_polygon_by_face(face_vertices.clone());
      // let ccw_vertices = windingsort::ccw_test(face_vertices.clone());
      for index in triangulated_face {
        for i in index {
          let vertex = face_vertices[i as usize].clone();
          // let vertex = ccw_vertices[i as usize];
          local_geometry.push(vertex.x);
          local_geometry.push(vertex.y);
          local_geometry.push(vertex.z);
        }
      }
    }
    
    // let face_data_string = serde_json::to_string(&face).unwrap(); // Serialize face_data
    // face_data_string

    // let extrude_data_string = serde_json::to_string(&extrude_data).unwrap(); // Serialize extrude_data
    // extrude_data_string

    let string_data = serde_json::to_string(&local_geometry).unwrap();
    string_data
  }
  
  #[wasm_bindgen]
  pub fn discard_geometry(&mut self) {
    // self.geometry.discard_geometry();
  }

  #[wasm_bindgen]
  pub fn outline_edges(&mut self) -> String {
    let mut outline_points: Vec<f64> = Vec::new();

    for edge in self.brep.edges.clone() {
      let start_index = edge[0] as usize;
      let end_index = edge[1] as usize;

      let start_point = self.brep.vertices[start_index].clone();
      let end_point = self.brep.vertices[end_index].clone();

      outline_points.push(start_point.x);
      outline_points.push(start_point.y);
      outline_points.push(start_point.z);

      outline_points.push(end_point.x);
      outline_points.push(end_point.y);
      outline_points.push(end_point.z);
    }

    let outline_data_string = serde_json::to_string(&outline_points).unwrap();
    outline_data_string
  }

  #[wasm_bindgen]
  pub fn get_brep_dump(&mut self) -> String {
    let brep_data_string = serde_json::to_string(&self.brep).unwrap();
    brep_data_string
  }
}