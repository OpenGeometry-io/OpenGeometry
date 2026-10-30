use crate::brep::{
    plane_boundary, unit, Builder, Frame3, GeometryError, Orientation, SurfaceGeometry,
};
use crate::math::{cross, norm, sub};

pub(super) struct ExtrudedLoop {
    pub(super) bottom_vertices: Vec<u32>,
    pub(super) top_vertices: Vec<u32>,
    pub(super) bottom_edges: Vec<u32>,
    pub(super) top_edges: Vec<u32>,
    pub(super) vertical_edges: Vec<u32>,
}

pub(super) struct SideFace {
    pub(super) frame: Frame3,
    pub(super) height: f64,
    pub(super) g: f64,
    pub(super) loop_index: usize,
    pub(super) index: usize,
    pub(super) next: usize,
    pub(super) bottom_from: u32,
    pub(super) bottom_to: u32,
    pub(super) top_from: u32,
    pub(super) top_to: u32,
}

pub(super) fn add_plane_side(
    builder: &mut Builder,
    profile: &ExtrudedLoop,
    side: &SideFace,
) -> Result<(), GeometryError> {
    let &SideFace {
        frame,
        height,
        g,
        loop_index,
        index,
        next,
        bottom_from,
        bottom_to,
        top_from,
        top_to,
    } = side;
    let origin = builder.brep.topology.vertices[bottom_from as usize].position;
    let destination = builder.brep.topology.vertices[bottom_to as usize].position;
    let edge_direction = unit(sub(destination, origin))?;
    let side_frame = Frame3::from_axis(
        origin,
        unit(cross(edge_direction, frame.z))?,
        edge_direction,
    )?;
    let uses = [
        (
            profile.bottom_edges[index],
            bottom_from,
            bottom_to,
            Orientation::Forward,
        ),
        (
            profile.vertical_edges[next],
            bottom_to,
            top_to,
            Orientation::Forward,
        ),
        (
            profile.top_edges[index],
            top_to,
            top_from,
            Orientation::Reverse,
        ),
        (
            profile.vertical_edges[index],
            top_from,
            bottom_from,
            Orientation::Reverse,
        ),
    ]
    .into_iter()
    .map(|(edge, from, to, sense)| plane_boundary(builder, side_frame, edge, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    builder.face(
        &format!("side-{loop_index}-{index}"),
        SurfaceGeometry::Plane { frame: side_frame },
        [[-g, norm(sub(destination, origin)) + g], [-g, height + g]],
        uses,
    )
}
