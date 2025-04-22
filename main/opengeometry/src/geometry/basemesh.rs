use crate::utility::openmath::{self, Geometry};
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use super::basegeometry;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct BaseMesh {
  id: String,
  geometry: basegeometry::BaseGeometry,
  brep: Geometry,
  pub is_from_extruded: bool,
  pub is_from_polygon: bool,
  pub position: openmath::Vector3D,
  pub rotation: openmath::Vector3D,
  pub scale: openmath::Vector3D,
  buffer: Vec<f64>
}

#[wasm_bindgen]
impl BaseMesh {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> BaseMesh {
    let geometry_id = id.clone();
    BaseMesh {
      id,
      geometry : basegeometry::BaseGeometry::new(geometry_id.clone()),
      brep : Geometry {
        vertices: Vec::new(),
        edges: Vec::new(),
        faces: Vec::new(),
      },
      is_from_extruded : false,
      is_from_polygon : false,
      position : openmath::Vector3D::create(0.0, 0.0, 0.0),
      rotation : openmath::Vector3D::create(0.0, 0.0, 0.0),
      scale : openmath::Vector3D::create(1.0, 1.0, 1.0),
      buffer : Vec::new()
    }
  }

  /**
   * Side view as of now
   */
  pub fn outline(&self) -> String {
    let mut outline_data = Vec::new();

    for face in self.brep.faces.clone() {
      for index in face {
        let vertex_start = self.brep.vertices[index as usize].clone();
        
        let mut vertex_end_index = 0;
        // Check if the next index is within bounds
        if vertex_end_index < self.brep.vertices.len() {
          vertex_end_index += 1;
        } else {
          vertex_end_index = 0;
        }

        let vertex_end = self.brep.vertices[vertex_end_index as usize].clone();
        let edge = {
          vec![vertex_start, vertex_end]
        };
        outline_data.push(edge);
      }
    }

    serde_json::to_string(&outline_data).unwrap()
  }
}

