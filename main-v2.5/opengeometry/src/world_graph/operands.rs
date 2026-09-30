use super::error::{ErrorCode, GraphError};
use super::graph::WorldGraph;
use crate::brep::{CurveGeometry, EdgeGeometry, Similarity3};
use crate::math::Point3;
use crate::operations::creating::{mapped_frame, ProfileLoop};
use crate::operations::invalid;

pub(super) fn path_points(
    graph: &WorldGraph,
    path: &str,
    target: &str,
) -> Result<Vec<Point3>, GraphError> {
    let mapping = graph.relative_placement(path, target)?;
    if mapping.scale.to_bits() != 1.0f64.to_bits() {
        return Err(GraphError::code(
            ErrorCode::InvalidOperand,
            "path has a relative scale",
        ));
    }
    let brep = graph.brep(path)?;
    let wire = brep
        .topology
        .wires
        .first()
        .ok_or_else(|| GraphError::code(ErrorCode::InvalidOperand, "path is not a wire"))?;
    if wire.is_closed || brep.topology.wires.len() != 1 {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "path must be one open wire",
        ));
    }
    let mut result = Vec::new();
    let mut cursor = wire.start_halfedge;
    for _ in 0..brep.topology.halfedges.len() {
        let halfedge = &brep.topology.halfedges[cursor as usize];
        let edge = &brep.topology.edges[halfedge.edge as usize];
        let EdgeGeometry::Curve { curve, .. } = edge.geometry else {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "path has a collapsed edge",
            ));
        };
        if !matches!(
            brep.geometry.curves[curve as usize],
            CurveGeometry::Line { .. }
        ) {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "path has a curved edge",
            ));
        }
        result.push(mapping.apply_point(brep.topology.vertices[halfedge.from as usize].position));
        if halfedge.next.is_none() {
            result.push(mapping.apply_point(brep.topology.vertices[halfedge.to as usize].position));
            break;
        }
        cursor = halfedge.next.ok_or_else(|| {
            GraphError::code(ErrorCode::InvalidTopology, "path ends unexpectedly")
        })?;
    }
    if result.len() < 2 {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "path is too short",
        ));
    }
    Ok(result)
}

pub(super) fn profile_loop(
    graph: &WorldGraph,
    profile: &str,
    target: &str,
) -> Result<ProfileLoop, GraphError> {
    let mapping = graph.relative_placement(profile, target)?;
    profile_loop_mapped(graph, profile, mapping)
}

fn profile_loop_mapped(
    graph: &WorldGraph,
    profile: &str,
    mapping: Similarity3,
) -> Result<ProfileLoop, GraphError> {
    if mapping.scale.to_bits() != 1.0f64.to_bits() {
        return Err(GraphError::code(
            ErrorCode::InvalidOperand,
            "profile has a relative scale",
        ));
    }
    let brep = graph.brep(profile)?;
    let wire = brep
        .topology
        .wires
        .first()
        .ok_or_else(|| GraphError::code(ErrorCode::InvalidOperand, "profile is not a wire"))?;
    if !wire.is_closed || brep.topology.wires.len() != 1 {
        return Err(invalid("profile must be one closed wire"));
    }
    let mut points = Vec::new();
    let mut cursor = wire.start_halfedge;
    for _ in 0..brep.topology.halfedges.len() {
        let halfedge = brep
            .topology
            .halfedges
            .get(cursor as usize)
            .ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "wire halfedge is missing")
            })?;
        let edge = &brep.topology.edges[halfedge.edge as usize];
        if let EdgeGeometry::Curve { curve, range } = edge.geometry {
            if let CurveGeometry::Circle { frame, radius } = brep.geometry.curves[curve as usize] {
                if brep.topology.halfedges.len() == 1
                    && (range.hi - range.lo - std::f64::consts::TAU).abs() <= 1e-12
                {
                    return Ok(ProfileLoop::Circle {
                        frame: mapped_frame(frame, mapping),
                        radius,
                    });
                }
            }
            if !matches!(
                brep.geometry.curves[curve as usize],
                CurveGeometry::Line { .. }
            ) {
                return Err(GraphError::code(
                    ErrorCode::UnsupportedGeometry,
                    "profile curve is not supported",
                ));
            }
        } else {
            return Err(invalid("profile contains a collapsed edge"));
        }
        points.push(mapping.apply_point(brep.topology.vertices[halfedge.from as usize].position));
        cursor = halfedge
            .next
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidTopology, "wire chain is open"))?;
        if cursor == wire.start_halfedge {
            break;
        }
    }
    if points.len() < 3 {
        return Err(invalid("profile has fewer than three vertices"));
    }
    Ok(ProfileLoop::Lines(points))
}
