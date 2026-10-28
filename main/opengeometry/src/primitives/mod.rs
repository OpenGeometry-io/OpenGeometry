mod box_boundary;
mod circle;
mod cuboid;
mod cylinder;
mod internal;
mod polyhedron;
mod polyline;
mod profile;
mod profile_wire;
mod rectangle;
mod revolved_section;
#[cfg(test)]
mod tests;
mod wire_points;

pub(super) use box_boundary::{
    add_box_corners, box_face_boundary, box_face_layout, BoxCorners, BoxFaceLayout,
};
pub use circle::arc_wire;
pub use cuboid::cuboid;
pub use cylinder::cylinder;
pub use internal::{
    annular_cylinder, arc_edged_extrusion, arc_edged_extrusion_with_holes, cone, cylinder_sector,
    cylinder_with_circular_hole, frustum, linear_extrusion, revolved_profile, sphere, torus,
};
pub use polyhedron::polyhedron;
pub use polyline::{polyline, polyline_with_keys};
pub use profile::ProfileEdge;
pub(super) use profile::{
    loop_inside, loops_touch, profile_area_twice, profile_edge_intersections, profile_point_inside,
    profile_self_intersects,
};
pub(super) use profile_wire::profile_segment_edges;
pub use profile_wire::{profile_wire_with_keys, ProfileSegment, SegmentArc};
pub use rectangle::{rectangle, rectangle_with_keys};
