/*
 * Simple Line
 * A Line is defined by two points.
 * Created on XZ plane.
 */

use crate::utility::openmath;
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGSimpleLine {
  id: String,
  points: Vec<openmath::Vector3D>
}

#[wasm_bindgen]
impl OGSimpleLine {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGSimpleLine {
    OGSimpleLine {
      id,
      // No more than 2 points for simple line, else it becomes a polyline
      points: Vec::with_capacity(2)
    }
  }

  #[wasm_bindgen]
  pub fn clone(&self) -> OGSimpleLine {
    OGSimpleLine {
      id: self.id.clone(),
      points: self.points.clone()
    }
  }

  #[wasm_bindgen]
  pub fn set_config(&mut self, start: openmath::Vector3D, end: openmath::Vector3D) {
    self.points.clear();
    self.points.push(start);
    self.points.push(end);
  }

  // Dispose
  #[wasm_bindgen]
  pub fn dispose_points(&mut self) {
    self.points.clear();
  }

  // TODO: Implement Destroy
  #[wasm_bindgen]
  pub fn destroy(&mut self) {
    self.points.clear();
  }

  // Get Points for the Circle
  #[wasm_bindgen]
  pub fn get_points(&self) -> String {
    serde_json::to_string(&self.points).unwrap()
  }

  pub fn get_raw_points(&self) -> Vec<openmath::Vector3D> {
    self.points.clone()
  }
}