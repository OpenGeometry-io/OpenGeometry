use super::error::{ErrorCode, GraphError};
use super::graph::WorldGraph;
use crate::brep::{BrepEnvelope, CurveGeometry, EdgeGeometry, HalfEdge, Orientation, Similarity3};
use crate::math::scale;
use crate::operations::creating::sweep::{Bend, SweepPath};
use crate::operations::creating::{mapped_frame, LoopArc, LoopEdge, ProfileLoop};

pub(super) fn sweep_path(
    graph: &WorldGraph,
    path: &str,
    target: &str,
) -> Result<SweepPath, GraphError> {
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
    let mut result = SweepPath {
        points: Vec::new(),
        bends: Vec::new(),
    };
    let mut cursor = wire.start_halfedge;
    for _ in 0..brep.topology.halfedges.len() {
        let halfedge = &brep.topology.halfedges[cursor as usize];
        result.bends.push(path_bend(&brep, halfedge, mapping)?);
        result
            .points
            .push(mapping.apply_point(brep.topology.vertices[halfedge.from as usize].position));
        if halfedge.next.is_none() {
            result
                .points
                .push(mapping.apply_point(brep.topology.vertices[halfedge.to as usize].position));
            break;
        }
        cursor = halfedge.next.ok_or_else(|| {
            GraphError::code(ErrorCode::InvalidTopology, "path ends unexpectedly")
        })?;
    }
    if result.points.len() < 2 {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "path is too short",
        ));
    }
    Ok(result)
}

fn path_bend(
    brep: &BrepEnvelope,
    halfedge: &HalfEdge,
    mapping: Similarity3,
) -> Result<Option<Bend>, GraphError> {
    let EdgeGeometry::Curve { curve, range } = brep.topology.edges[halfedge.edge as usize].geometry
    else {
        return Err(invalid("path has a collapsed edge"));
    };
    match brep.geometry.curves[curve as usize] {
        CurveGeometry::Line { .. } => Ok(None),
        CurveGeometry::Circle { frame, .. } => {
            let turn = match halfedge.geometry_use.sense {
                Orientation::Forward => 1.0,
                Orientation::Reverse => -1.0,
            };
            Ok(Some(Bend {
                centre: mapping.apply_point(frame.origin),
                axis: scale(mapping.apply_vector(frame.z), turn),
                angle: range.hi - range.lo,
            }))
        }
        _ => Err(invalid("path has a curved edge")),
    }
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
    if let Some(circle) = lone_circle(&brep, mapping) {
        return Ok(circle);
    }
    let mut edges = Vec::new();
    let mut cursor = wire.start_halfedge;
    for _ in 0..brep.topology.halfedges.len() {
        let halfedge = brep
            .topology
            .halfedges
            .get(cursor as usize)
            .ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "wire halfedge is missing")
            })?;
        edges.push(loop_edge(&brep, halfedge, mapping)?);
        cursor = halfedge
            .next
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidTopology, "wire chain is open"))?;
        if cursor == wire.start_halfedge {
            break;
        }
    }
    if edges.iter().any(|edge| edge.arc.is_some()) {
        return Ok(ProfileLoop::Curves(edges));
    }
    if edges.len() < 3 {
        return Err(invalid("profile has fewer than three vertices"));
    }
    Ok(ProfileLoop::Lines(
        edges.into_iter().map(|edge| edge.from).collect(),
    ))
}

fn lone_circle(brep: &BrepEnvelope, mapping: Similarity3) -> Option<ProfileLoop> {
    let [halfedge] = brep.topology.halfedges.as_slice() else {
        return None;
    };
    let EdgeGeometry::Curve { curve, range } =
        brep.topology.edges.get(halfedge.edge as usize)?.geometry
    else {
        return None;
    };
    match brep.geometry.curves.get(curve as usize)? {
        CurveGeometry::Circle { frame, radius }
            if (range.hi - range.lo - std::f64::consts::TAU).abs() <= 1e-12 =>
        {
            Some(ProfileLoop::Circle {
                frame: mapped_frame(*frame, mapping),
                radius: *radius,
            })
        }
        _ => None,
    }
}

fn loop_edge(
    brep: &BrepEnvelope,
    halfedge: &HalfEdge,
    mapping: Similarity3,
) -> Result<LoopEdge, GraphError> {
    let EdgeGeometry::Curve { curve, .. } = brep.topology.edges[halfedge.edge as usize].geometry
    else {
        return Err(invalid("profile contains a collapsed edge"));
    };
    let arc = match brep.geometry.curves[curve as usize] {
        CurveGeometry::Line { .. } => None,
        CurveGeometry::Circle { frame, radius } => Some(LoopArc {
            center: mapping.apply_point(frame.origin),
            axis: mapping.apply_vector(frame.z),
            radius,
            counter_clockwise: matches!(halfedge.geometry_use.sense, Orientation::Forward),
        }),
        _ => {
            return Err(GraphError::code(
                ErrorCode::UnsupportedGeometry,
                "profile curve is not supported",
            ))
        }
    };
    Ok(LoopEdge {
        from: mapping.apply_point(brep.topology.vertices[halfedge.from as usize].position),
        arc,
    })
}

fn invalid(message: impl Into<String>) -> GraphError {
    GraphError::code(ErrorCode::InvalidParameter, message)
}
