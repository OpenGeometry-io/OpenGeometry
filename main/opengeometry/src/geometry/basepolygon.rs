use crate::operations::extrude::extrude_polygon_by_buffer_geometry;
use crate::operations::triangulate::triangulate_polygon_buffer_geometry;
use crate::operations::windingsort;
use crate::primitives;
use crate::utility::openmath::Vector3D;
use crate::{operations::triangulate, utility::openmath};
use crate::geometry::basegeometry;
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct BasePolygon {
  id: String,
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
  // Why Getter and Setter - https://github.com/rustwasm/wasm-bindgen/issues/1775
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  // Add the ability to create polygon with list of verticies passed in constructor itself
  // as of now use add_vertices method to push all vertices at once
  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> BasePolygon {
    BasePolygon {
      id: id.clone(),
      geometry : basegeometry::BaseGeometry::new(id.clone()),
      extruded : false,
      is_polygon : false,
      position : openmath::Vector3D::create(0.0, 0.0, 0.0),
      rotation : openmath::Vector3D::create(0.0, 0.0, 0.0),
      scale : openmath::Vector3D::create(1.0, 1.0, 1.0),
      buffer : Vec::new()
    }
  }

  #[wasm_bindgen]
  pub fn new_with_circle(circle_arc: primitives::circle::CircleArc) -> BasePolygon {
    let mut polygon = BasePolygon::new(circle_arc.id());
    // discard the last point as it is same as the first point
    let mut circle_arc_points = circle_arc.get_raw_points();
    circle_arc_points.pop();
    polygon.add_vertices(circle_arc_points);
    polygon.triangulate();
    polygon
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
  pub fn triangulate(&mut self) -> String {
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

    serde_json::to_string(&self.buffer).unwrap()
  }

  #[wasm_bindgen]
  pub fn get_buffer_flush(&self) -> String {
    serde_json::to_string(&self.buffer).unwrap()
  }

  #[wasm_bindgen]
  pub fn clear_vertices(&mut self) {
    self.geometry.reset_geometry();
  }

  #[wasm_bindgen]
  pub fn reset_polygon(&mut self) {
    // Reset the geometry
  }

  #[wasm_bindgen]
  pub fn extrude_by_height(&mut self, height: f64) -> String {
    self.extruded = true;
    let extruded_raw = extrude_polygon_by_buffer_geometry(self.geometry.clone(), height);
    
    let faces = extruded_raw.faces;
    let vertices = extruded_raw.vertices;

    let mut generated: Vec<f64> = Vec::new();

    for face in faces {
      let mut face_vertices: Vec<Vector3D> = Vec::new();
      for index in face {
        face_vertices.push(vertices[index as usize].clone());
      }

      let triangulated_face = triangulate::triangulate_polygon_by_face(face_vertices.clone());
      for index in triangulated_face {
        for i in index {
          let vertex = face_vertices[i as usize];
          generated.push(vertex.x);
          generated.push(vertex.y);
          generated.push(vertex.z);
        }
      }
    }

    serde_json::to_string(&generated).unwrap()
  }
}
