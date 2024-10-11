use crate::utility::openmath;
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use super::basegeometry;

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct BaseMesh {
  pub id: u32,
  geometry: basegeometry::BaseGeometry,
  pub extruded: bool,
  pub is_polygon: bool,
  pub position: openmath::Vector3D,
  pub rotation: openmath::Vector3D,
  pub scale: openmath::Vector3D,
  buffer: Vec<f64>
}
