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
  pub mod intersect;
  pub mod extrude;
}

pub mod utility {
  pub mod openmath;
}

pub mod primitives {
  pub mod circle;
}
