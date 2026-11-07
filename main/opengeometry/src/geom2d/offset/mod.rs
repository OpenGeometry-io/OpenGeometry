mod curved_ring;
mod cutters;
mod pinch;
mod polyline;
mod ring;
mod source_edge;
#[cfg(test)]
mod tests;
mod tidy;

pub(crate) use curved_ring::{inset_curved_region, inset_tolerance, offset_curved_ring_variable};
pub(crate) use pinch::pinched;
pub(crate) use polyline::{offset_path, offset_polyline_regions, OffsetOptions};
pub(crate) use ring::{offset_ring_variable, OffsetError, OffsetHole};
pub(crate) use source_edge::offset_from;
pub(crate) use tidy::oriented_ring;
