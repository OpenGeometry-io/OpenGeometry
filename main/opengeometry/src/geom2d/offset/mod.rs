mod polyline;
mod ring;
#[cfg(test)]
mod tests;

pub(crate) use polyline::{offset_path, offset_polyline_regions, OffsetOptions};
pub(crate) use ring::{offset_ring_variable, OffsetError, OffsetHole};
