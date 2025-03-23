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
  pub fn new(id: String) -> CircleArc {
    CircleArc {
      id,
      center: openmath::Vector3D::create(0.0, 0.0, 0.0),
      radius: 1.0,
      start_angle: 0.0,
      end_angle: 2.0 * std::f64::consts::PI,
      segments: 32,
      points: Vec::new()
    }
  }

  #[wasm_bindgen]
  pub fn clone(&self) -> CircleArc {
    CircleArc {
      id: self.id.clone(),
      center: self.center.clone(),
      radius: self.radius,
      start_angle: self.start_angle,
      end_angle: self.end_angle,
      segments: self.segments,
      points: self.points.clone()
    }
  }

  #[wasm_bindgen]
  pub fn set_config(&mut self, center: openmath::Vector3D, radius: f64, start_angle: f64, end_angle: f64, segments: u32) {
    self.center = center;
    self.radius = radius;
    self.start_angle = start_angle;
    self.end_angle = end_angle;
    self.segments = segments;
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
  }

  #[wasm_bindgen]
  pub fn update_center(&mut self, center: openmath::Vector3D) {
    self.destroy();
    self.center = center;
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