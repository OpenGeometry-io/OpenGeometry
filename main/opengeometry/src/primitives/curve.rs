/**
 * Copyright (c) 2025, OpenGeometry. All rights reserved.
 * Curve Primitive for OpenGeometry.
 *
 * A Curve is a continuous and smooth flowing line defined by a set of control points.
 * It can be used to create complex shapes and paths in 3D space.
 * Created with a set of control points.
 **/

 // Ref - https://docs.blender.org/manual/en/latest/modeling/curves/structure.html
 // Ref - https://help.autodesk.com/view/ACD/2025/ENU/?guid=GUID-5E7D51E2-1595-4E0C-85F8-2D7CBD166A08

use crate::brep::{Edge, Face, Brep, Vertex};
use crate::utility::bgeometry::BufferGeometry;
#[cfg(feature="wasm")] use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use openmaths::Vector3;
use uuid::Uuid; 

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGCurve {
  id: String,
  control_points: Vec<Vector3>,
  brep: Brep,
  geometry: BufferGeometry, // TODO: Use BufferGeometry to store the triangulated geometry
} 

#[wasm_bindgen]
impl OGCurve {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  } 

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGCurve {
    OGCurve {
      id,
      control_points: Vec::new(),
      brep: Brep::new(),
      geometry: BufferGeometry::new(),
    }
  } 

  #[wasm_bindgen]
  pub fn set_config(&mut self, control_points: Vec<Vector3>) {
    self.brep.clear();
    self.control_points = control_points;
  } 

  #[wasm_bindgen]
  pub fn generate_geometry(&mut self) {
    // Create vertices for each control point
    for point in &self.control_points {
      self.brep.vertices.push(Vertex::new(self.brep.get_vertex_count() as u32, *point));
    }

    // Create edges between consecutive control points
    for i in 0..self.control_points.len() - 1 {
      let start_vertex = self.brep.vertices[i].clone();
      let end_vertex = self.brep.vertices[i + 1].clone();
      let edge = Edge::new(self.brep.get_edge_count() as u32, start_vertex.id, end_vertex.id);
      self.brep.edges.push(edge);
    }
  } 

  // Dispose resources
  #[wasm_bindgen]
  pub fn dispose(&mut self) {
    self.brep.clear();
    self.control_points.clear();
    self.geometry.clear();
  }
}