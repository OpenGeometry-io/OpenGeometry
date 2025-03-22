/*
 * Circle Curve
 * A circle is defined by a center point and a radius.
 * Created on XZ plane. 
 */

// TODO: What if we create the Circle using the Formula for Angles.

use crate::utility::openmath;
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct CircleArc {
  id: String,
  center: openmath::Vector3D,
  radius: f64,
  start_angle: f64,
  end_angle: f64,
  segments: u32,
  points: Vec<openmath::Vector3D>
}

#[wasm_bindgen]
impl CircleArc {
  #[wasm_bindgen(setter)]
  pub fn set_id(&mut self, id: String) {
    self.id = id;
  }

  #[wasm_bindgen(getter)]
  pub fn id(&self) -> String {
    self.id.clone()
  }

  #[wasm_bindgen(constructor)]
  pub fn new(id: String, center: openmath::Vector3D, radius: f64, start_angle: f64, end_angle: f64, segments: u32) -> CircleArc {
    CircleArc {
      id,
      center,
      radius,
      start_angle,
      end_angle,
      segments,
      points: Vec::new()
    }
  }

  #[wasm_bindgen]
  pub fn generate_points(&mut self) {
    let mut angle = self.start_angle;
    let angle_diff = (self.end_angle - self.start_angle) / self.segments as f64;
    for _ in 0..self.segments + 1 {
      let x = self.center.x + self.radius * angle.cos();
      let y = self.center.y;
      let z = self.center.z + self.radius * angle.sin();
      self.points.push(openmath::Vector3D::create(x, y, z));
      angle += angle_diff;
    }
  }

  #[wasm_bindgen]
  pub fn update_radius(&mut self, radius: f64) {
    self.destroy();
    self.radius = radius;
    self.generate_points();
  }

  #[wasm_bindgen]
  pub fn update_center(&mut self, center: openmath::Vector3D) {
    self.destroy();
    self.center = center;
    self.generate_points();
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

  // TODO: Implement Get Buffer Geometry for the Circle
  // Get Buffer Geometry for the Circle
}