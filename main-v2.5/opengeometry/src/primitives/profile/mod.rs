mod arc;
mod polygon;

pub(super) use arc::validate_arc_profile_loop;
pub use arc::ProfileEdge;
pub(crate) use arc::{arc_profile_contains, profile_edge_intersections};
pub(crate) use polygon::{loops_touch, profile_point_inside};
pub(super) use polygon::{orient_profile_loop, validate_profile_loop};
