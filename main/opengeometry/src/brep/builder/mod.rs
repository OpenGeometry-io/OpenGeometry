mod boundary;
mod brep_builder;
#[cfg(test)]
mod tests;
mod wire;

pub use boundary::{boundary, plane_boundary, uv_conic, uv_line, UvCurve};
pub(crate) use boundary::{dimensions, padded_uv_bounds};
pub use brep_builder::{Builder, Use};
pub(crate) use wire::WireUse;
