mod arc;
mod polygon;

pub use arc::ProfileEdge;
pub(super) use arc::{arc_profile_contains, profile_edge_intersections, validate_arc_profile_loop};
pub(super) use polygon::{
    loops_touch, orient_profile_loop, profile_point_inside, validate_profile_loop,
};
