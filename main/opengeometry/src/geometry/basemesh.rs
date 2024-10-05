use crate::utility::openmath;
use std::path;

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct BaseMesh {
  pub id: u32,
  vertices: Vec<openmath::Vector3D>,
  indices: Vec<u32>,
  normals: Vec<f32>
}
