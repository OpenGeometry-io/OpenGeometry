use core::num;
use std::{collections::HashMap, hash::Hash, ptr::null};

#[cfg(feature="wasm")] use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

use openmaths::Vector3;

// pub fn add_extrude_in_up(&self, height: f64, up_vector: Vector3D) -> Vector3D {
//   Vector3D {
//     x: self.x + up_vector.x * height,
//     y: self.y + up_vector.y * height,
//     z: self.z + up_vector.z * height
//   }
// }

// pub fn cross(&self, other: &Vector3D) -> Vector3D {
//   Vector3D {
//     x: self.y * other.z - self.z * other.y,
//     y: self.z * other.x - self.x * other.z,
//     z: self.x * other.y - self.y * other.x
//   }
// }

// pub fn dot(&self, other: &Vector3D) -> f64 {
//   self.x * other.x + self.y * other.y + self.z * other.z
// }


#[cfg_attr(feature="wasm", wasm_bindgen)]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct ColorRGB {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

impl ColorRGB {
  pub fn new(r: u8, g: u8, b: u8) -> ColorRGB {
    ColorRGB { r, g, b }
  }

  pub fn to_hex(&self) -> String {
    format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
  }
}

#[cfg_attr(feature="wasm", wasm_bindgen)]
pub struct Color {
  hex: String
}

impl Color {
  pub fn new(hex: String) -> Color {
    Color { hex }
  }

  pub fn to_rgba(&self) -> Result<ColorRGB, String> {
    let hex = self.hex.trim_start_matches('#');
    let len = hex.len();

    if len != 6 && len != 8 {
        return Err("Hex string must be in the format #RRGGBB or #RRGGBBAA".to_string());
    }

    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid red component")?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid green component")?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid blue component")?;

    Ok(ColorRGB { r, g, b })
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Geometry {
  pub vertices: Vec<Vector3>,
  pub edges: Vec<Vec<u8>>,
  pub faces: Vec<Vec<u8>>,
}

impl Geometry {
  pub fn new() -> Geometry {
    Geometry {
      vertices: Vec::new(),
      edges: Vec::new(),
      faces: Vec::new(),
    }
  }

  pub fn get_geometry(&self) -> String {
    // serialize geometry
    let serialized = serde_json::to_string(&self).unwrap();
    serialized
  }

  pub fn get_geometry_raw(&self) -> Geometry {
    self.clone()
  }

  pub fn get_faces(&self) -> Vec<Vec<u8>> {
    self.faces.clone()
  }

  pub fn add_edge(&mut self, edge: Vec<u8>) {
    self.edges.push(edge);
  }

  pub fn clear(&mut self) {
    self.vertices.clear();
    self.edges.clear();
    self.faces.clear();
  }
}


// Brep Geometry with Holes
#[derive(Clone, Serialize, Deserialize)]
pub struct Geometry_Holes {
  pub vertices: Vec<Vector3>,
  pub edges: Vec<Vec<u8>>,
  pub faces: Vec<Vec<u8>>,
  pub holes: Vec<Vec<Vector3>>,
  pub face_holes_map: HashMap<u8, Vec<u8>>,
  pub is_ccw_last_face: bool,
  pub face_length: usize
}

impl Geometry_Holes {
  pub fn new() -> Geometry_Holes {
    Geometry_Holes {
      vertices: Vec::new(),
      edges: Vec::new(),
      faces: Vec::new(),
      holes: Vec::new(),
      face_holes_map: HashMap::new(),
      is_ccw_last_face: false,
      face_length: 0,
    }
  }

  pub fn get_geometry(&self) -> String {
    // serialize geometry
    let serialized = serde_json::to_string(&self).unwrap();
    serialized
  }
}

#[cfg(feature="wasm")]
#[wasm_bindgen]
impl ColorRGB {
  #[wasm_bindgen(constructor)]
  pub fn wasm_new(r: u8, g: u8, b: u8) -> ColorRGB { ColorRGB::new(r, g, b) }
}

#[cfg(feature="wasm")]
#[wasm_bindgen]
impl Color {
  #[wasm_bindgen(constructor)]
  pub fn wasm_new(hex: String) -> Color { Color::new(hex) }
  pub fn wasm_to_rgba(&self) -> Result<ColorRGB, String> { self.to_rgba() }
}
