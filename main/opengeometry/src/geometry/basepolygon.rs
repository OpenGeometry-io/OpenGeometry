use crate::operations::triangulate::triangulate_polygon_buffer_geometry;
use crate::operations::windingsort;
use crate::{operations::triangulate, utility::openmath};
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
  pub scale: openmath::Vector3D,
  buffer: Vec<f64>
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
      scale : openmath::Vector3D::create(1.0, 1.0, 1.0),
      buffer : Vec::new()
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

  // #[wasm_bindgen]
  // pub fn triangulate(&mut self) -> String {
  //   if self.is_polygon {
  //     // Polygon is already triangulated, destroy the previous triangulation
  //     return String::from("Polygon is already triangulated");
  //   }

  //   if self.geometry.get_vertices().len() < 3 {
  //     return String::from("Polygon should have atleast 3 vertices");
  //   }

  //   self.is_polygon = true;
  //   triangulate_polygon_buffer_geometry(self.geometry.clone())
  // }

  #[wasm_bindgen]
  pub fn triangulate(&mut self) {
    if self.is_polygon {
      // Polygon is already triangulated, destroy the previous triangulation
      // return String::from("Polygon is already triangulated");
    }

    if self.geometry.get_vertices().len() < 3 {
      // return String::from("Polygon should have atleast 3 vertices");
    }

    self.is_polygon = true;
    let indices = triangulate_polygon_buffer_geometry(self.geometry.clone());

    let ccw_vertices = windingsort::ccw_test(self.geometry.get_vertices());
    
    for index in indices {
      for i in index {
        let vertex = ccw_vertices[i as usize];
        self.buffer.push(vertex.x);
        self.buffer.push(vertex.y);
        self.buffer.push(vertex.z);
      }
    }
  }

  #[wasm_bindgen]
  pub fn get_buffer_flush(&self) -> String {
    serde_json::to_string(&self.buffer).unwrap()
  }
}
