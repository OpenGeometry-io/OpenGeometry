use core::num;
use std::{collections::HashMap, hash::Hash};

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

#[wasm_bindgen]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Vector3D {
  pub x: f64,
  pub y: f64,
  pub z: f64,
}

#[wasm_bindgen]
impl Vector3D {
  #[wasm_bindgen(constructor)]
  pub fn create(x: f64, y: f64, z: f64) -> Vector3D {
    Vector3D { x, y, z }
  }

  pub fn update(&mut self, x: f64, y: f64, z: f64) {
    self.x = x;
    self.y = y;
    self.z = z;
  }

  pub fn add(&self, other: &Vector3D) -> Vector3D {
    Vector3D {
      x: self.x + other.x,
      y: self.y + other.y,
      z: self.z + other.z
    }
  }

  pub fn subtract(&self, other: &Vector3D) -> Vector3D {
    Vector3D {
      x: self.x - other.x,
      y: self.y - other.y,
      z: self.z - other.z
    }
  }

  pub fn add_scalar(&self, scalar: f64) -> Vector3D {
    Vector3D {
      x: self.x + scalar,
      y: self.y + scalar,
      z: self.z + scalar
    }
  }

  pub fn subtract_scalar(&self, scalar: f64) -> Vector3D {
    Vector3D {
      x: self.x - scalar,
      y: self.y - scalar,
      z: self.z - scalar
    }
  }

  pub fn add_extrude_in_up(&self, height: f64, up_vector: Vector3D) -> Vector3D {
    Vector3D {
      x: self.x + up_vector.x * height,
      y: self.y + up_vector.y * height,
      z: self.z + up_vector.z * height
    }
  }

  pub fn cross(&self, other: &Vector3D) -> Vector3D {
    Vector3D {
      x: self.y * other.z - self.z * other.y,
      y: self.z * other.x - self.x * other.z,
      z: self.x * other.y - self.y * other.x
    }
  }

  pub fn dot(&self, other: &Vector3D) -> f64 {
    self.x * other.x + self.y * other.y + self.z * other.z
  }

  pub fn clone(&self) -> Vector3D {
    Vector3D {
      x: self.x,
      y: self.y,
      z: self.z
    }
  }
}

#[wasm_bindgen]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Matrix3D {
  pub m11: f64, pub m12: f64, pub m13: f64,
  pub m21: f64, pub m22: f64, pub m23: f64,
  pub m31: f64, pub m32: f64, pub m33: f64,
}

#[wasm_bindgen]
impl Matrix3D {
  #[wasm_bindgen(constructor)]
  pub fn set(
    m11: f64, m12: f64, m13: f64,
    m21: f64, m22: f64, m23: f64,
    m31: f64, m32: f64, m33: f64,
  ) -> Matrix3D {
    Matrix3D { m11, m12, m13, m21, m22, m23, m31, m32, m33 }
  }

  pub fn add(&self, other: &Matrix3D) -> Matrix3D {
    Matrix3D {
      m11: self.m11 + other.m11, m12: self.m12 + other.m12, m13: self.m13 + other.m13,
      m21: self.m21 + other.m21, m22: self.m22 + other.m22, m23: self.m23 + other.m23,
      m31: self.m31 + other.m31, m32: self.m32 + other.m32, m33: self.m33 + other.m33,
    }
  }

  pub fn subtract(&self, other: &Matrix3D) -> Matrix3D {
    Matrix3D {
      m11: self.m11 - other.m11, m12: self.m12 - other.m12, m13: self.m13 - other.m13,
      m21: self.m21 - other.m21, m22: self.m22 - other.m22, m23: self.m23 - other.m23,
      m31: self.m31 - other.m31, m32: self.m32 - other.m32, m33: self.m33 - other.m33,
    }
  }
}

#[wasm_bindgen]
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct ColorRGB {
  pub r: u8,
  pub g: u8,
  pub b: u8,
}

#[wasm_bindgen]
impl ColorRGB {
  #[wasm_bindgen(constructor)]
  pub fn new(r: u8, g: u8, b: u8) -> ColorRGB {
    ColorRGB { r, g, b }
  }

  pub fn to_hex(&self) -> String {
    format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
  }
}

#[wasm_bindgen]
pub struct Color {
  hex: String
}

#[wasm_bindgen]
impl Color {
  #[wasm_bindgen(constructor)]
  pub fn new(hex: String) -> Color {
    Color { hex }
  }

  #[wasm_bindgen]
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
  pub vertices: Vec<Vector3D>,
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
}


// Brep Geometry with Holes
#[derive(Clone, Serialize, Deserialize)]
pub struct Geometry_Holes {
  pub vertices: Vec<Vector3D>,
  pub edges: Vec<Vec<u8>>,
  pub faces: Vec<Vec<u8>>,
  pub holes: Vec<Vec<Vector3D>>,
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
