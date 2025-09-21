use wasm_bindgen::prelude::*;

pub mod geometry {
  pub mod basegeometry;
  pub mod basemesh;
  pub mod basepolygon;
  pub mod basegroup;
  pub mod triangle;
  pub mod baseflatmesh;
}

pub mod operations {
  pub mod triangulate;
  pub mod windingsort;
  pub mod extrude;
  pub mod triangulate_brep;
  pub mod boolean;
}

pub mod utility {
  pub mod geometry;
  pub mod bgeometry;
}

pub mod primitives {
  pub mod arc;
  pub mod line;
  pub mod polyline;
  pub mod rectangle;
  pub mod cylinder;
  pub mod polygon;
  pub mod cylinder2;
  pub mod cube;
}

pub mod brep;

// Re-export boolean operations for WASM
pub use operations::boolean::wasm_bindings::{OGBooleanEngine, OGBrep};

// v0.3.0
// mod brep_ds {
//   mod vertex;
//   // mod halfedge;
//   mod edge;
//   mod face;
//   pub mod brep;
// }

