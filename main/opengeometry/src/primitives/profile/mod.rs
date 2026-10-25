mod arc;
mod polygon;

pub(super) use arc::validate_arc_profile_loop;
pub use arc::ProfileEdge;
pub(crate) use arc::{
    loop_inside, profile_area_twice, profile_edge_intersections, profile_self_intersects,
};
pub(crate) use polygon::{loops_touch, profile_point_inside};
pub(super) use polygon::{orient_profile_loop, validate_profile_loop};
