pub mod geometry {
  pub mod basegeometry;
  pub mod triangle;
}

pub mod operations {
  pub mod triangulate;
  pub mod windingsort;
  pub mod extrude;
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
  pub mod polygon;
  pub mod cylinder;
  pub mod cuboid;
}

pub mod brep;

// Drawing abstraction for exports (PDF, SVG, DXF)
pub mod drawing;

// Export modules
pub mod export;

// v0.3.0
// mod brep_ds {
//   mod vertex;
//   // mod halfedge;
//   mod edge;
//   mod face;
//   pub mod brep;
// }

