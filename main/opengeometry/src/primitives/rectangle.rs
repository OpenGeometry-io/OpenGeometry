/*
 * Rectange
 * A Simple OGRectangle defined by width and breadth.
 * Created on XZ plane.
 */

use openmaths::Vector3;
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct OGRectangle {
  id: String,
  center: Vector3,
  width: f64,
  breadth: f64,
  points: Vec<Vector3>
}

#[wasm_bindgen]
impl OGRectangle {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String) -> OGRectangle {
    OGRectangle {
      id,
      center: Vector3::new(0.0, 0.0, 0.0),
      width: 1.0,
      breadth: 1.0,
      points: Vec::new()
    }
  }

  #[wasm_bindgen]
  pub fn clone(&self) -> OGRectangle {
    OGRectangle {
      id: self.id.clone(),
      center: self.center.clone(),
      width: self.width,
      breadth: self.breadth,
      points: self.points.clone()
    }
  }

  #[wasm_bindgen]
  pub fn set_config(&mut self, center: Vector3, width: f64, breadth: f64) {
    self.center = center;
    self.width = width;
    self.breadth = breadth;
  }

  #[wasm_bindgen]
  pub fn generate_points(&mut self) {
    let half_width = self.width / 2.0;
    let half_breadth = self.breadth / 2.0;

    let p1 = Vector3::new(self.center.x - half_width, self.center.y, self.center.z - half_breadth);
    let p2 = Vector3::new(self.center.x + half_width, self.center.y, self.center.z - half_breadth);
    let p3 = Vector3::new(self.center.x + half_width, self.center.y, self.center.z + half_breadth);
    let p4 = Vector3::new(self.center.x - half_width, self.center.y, self.center.z + half_breadth);

    self.points.clear();
    self.points.push(p1);
    self.points.push(p2);
    self.points.push(p3);
    self.points.push(p4);
    self.points.push(p1); // Close the loop
  }

  #[wasm_bindgen]
  pub fn update_width(&mut self, width: f64) {
    self.destroy();
    self.width = width;
  }

  #[wasm_bindgen]
  pub fn update_breadth(&mut self, breadth: f64) {
    self.destroy();
    self.breadth = breadth;
  }

  #[wasm_bindgen]
  pub fn update_center(&mut self, center: Vector3) {
    self.destroy();
    self.center = center;
  }

  #[wasm_bindgen]
  pub fn dispose_points(&mut self) {
    self.points.clear();
  }

  #[wasm_bindgen]
  pub fn destroy(&mut self) {
    self.points.clear();
  }

  // Get Points for the Circle
  #[wasm_bindgen]
  pub fn get_points(&self) -> String {
    serde_json::to_string(&self.points).unwrap()
  }

  pub fn get_raw_points(&self) -> Vec<Vector3> {
    self.points.clone()
  }
}