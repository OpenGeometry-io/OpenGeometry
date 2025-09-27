use crate::utility::geometry::{self, Geometry};
#[cfg(feature="wasm")] use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use openmaths::Vector3;
use super::basegeometry;

#[cfg_attr(feature="wasm", wasm_bindgen)]
#[derive(Clone, Serialize, Deserialize)]
pub struct BaseMesh {
  id: String,
  geometry: basegeometry::BaseGeometry,
  brep: Geometry,
  pub is_from_extruded: bool,
  pub is_from_polygon: bool,
  pub position: Vector3,
  pub rotation: Vector3,
  pub scale: Vector3,
  buffer: Vec<f64>
}

impl BaseMesh {
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  pub fn id(&self) -> String {
    self.id.clone()
  }

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
      position : Vector3::new(0.0, 0.0, 0.0),
      rotation : Vector3::new(0.0, 0.0, 0.0),
      scale : Vector3::new(1.0, 1.0, 1.0),
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

  pub fn generate_geometry(&mut self) {}
}

// WASM-specific implementation
#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl BaseMesh {
  #[wasm_bindgen(constructor)]
  pub fn new_wasm(id: String) -> BaseMesh {
    BaseMesh::new(id)
  }

  #[wasm_bindgen(setter)]
  pub fn set_id_wasm(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id_wasm(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen]
  pub fn generate_geometry_wasm(&mut self) {
    self.generate_geometry();
  }

  #[wasm_bindgen]
  pub fn get_geometry_serialized_wasm(&self) -> String {
    // Return empty serialized geometry for now
    "[]".to_string()
  }
}

