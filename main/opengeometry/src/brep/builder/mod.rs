mod boundary;
mod brep_builder;
#[cfg(test)]
mod tests;
mod wire;

pub(crate) use boundary::{boundary, dimensions, padded_uv_bounds, plane_boundary, uv_line};
pub(crate) use brep_builder::{Builder, Use};
pub(crate) use wire::WireUse;
