use crate::utility::openmath;
use crate::geometry::basegeometry;
use std::path;

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct BasePolygon {
  pub id: u32,
  geometry: basegeometry::BaseGeometry,
  pub extruded: bool,
  pub is_polygon: bool,
  pub position: openmath::Vector3D,
  pub rotation: openmath::Vector3D,
  pub scale: openmath::Vector3D
}

/**
 * A Polygon created with a `id` will have a BaseGeometry with same `id`. Feels like a good decision as of now.
 */

#[wasm_bindgen]
impl BasePolygon {
  // Add the ability to create polygon with list of verticies passed in constructor itself
  // as of now use add_vertices method to push all vertices at once
  #[wasm_bindgen(constructor)]
  pub fn new(id: u32) -> BasePolygon {
    BasePolygon {
      id,
      geometry : basegeometry::BaseGeometry::new(id),
      extruded : false,
      is_polygon : false,
      position : openmath::Vector3D::create(0.0, 0.0, 0.0),
      rotation : openmath::Vector3D::create(0.0, 0.0, 0.0),
      scale : openmath::Vector3D::create(1.0, 1.0, 1.0)
    }
  }

  #[wasm_bindgen]
  pub fn add_vertices(&mut self, vertices: Vec<openmath::Vector3D>) {
    self.geometry.add_vertices(vertices);
  }
  
  #[wasm_bindgen]
  pub fn add_vertex(&mut self, vertex: openmath::Vector3D) {
    self.geometry.add_vertex(vertex);
    
    // If more than 3 vertices are added, then the polygon is created
    if self.geometry.get_vertices().len() > 2 {
      self.is_polygon = true;
    }
  }

  #[wasm_bindgen]
  pub fn triangulate(&mut self) {
    if !self.is_polygon {
      return;
    }
  }

  #[wasm_bindgen]
  pub fn get_buffer(&self) -> String {
    serde_json::to_string(&self.geometry).unwrap()
  }
}