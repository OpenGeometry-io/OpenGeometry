use super::path::{check_path_ends, distance, parameter, PolysolidError, PolysolidPath};
use crate::primitives::profile::{
    profile_edge_intersections, profile_self_intersects, ProfileEdge,
};
use std::f64::consts::TAU;

const TANGENT_TOLERANCE: f64 = 1.0e-9;

pub(super) fn check_path(
    path: &PolysolidPath,
    edges: &[ProfileEdge],
    g: f64,
) -> Result<(), PolysolidError> {
    check_path_ends(
        path,
        if path.closed { 2 } else { 1 },
        "polysolid path has too few segments",
        g,
    )?;
    for (index, edge) in edges.iter().enumerate() {
        let to = if path.closed && index + 1 == edges.len() {
            path.start
        } else {
            path.segments[index].to
        };
        check_segment(edge, to, index, g)?;
    }
    if profile_self_intersects(edges, path.closed, g) {
        return Err(parameter("polysolid path crosses itself".into()));
    }
    Ok(())
}

fn check_segment(
    edge: &ProfileEdge,
    to: [f64; 2],
    index: usize,
    g: f64,
) -> Result<(), PolysolidError> {
    if distance(edge.endpoints().0, to) <= 4.0 * g {
        return Err(parameter(format!(
            "polysolid path segment {index} has zero length"
        )));
    }
    let &ProfileEdge::Arc {
        center,
        radius,
        sweep_angle,
        ..
    } = edge
    else {
        return Ok(());
    };
    if radius <= 4.0 * g || (distance(to, center) - radius).abs() > g / 4.0 {
        return Err(parameter(format!(
            "polysolid path segment {index} is an arc that does not end on its circle"
        )));
    }
    if sweep_angle.abs() >= TAU || radius * sweep_angle.abs() <= 4.0 * g {
        return Err(parameter(format!(
            "polysolid path segment {index} is an arc below geometric resolution"
        )));
    }
    Ok(())
}

pub(super) fn check_tangent_joints(
    edges: &[ProfileEdge],
    closed: bool,
    closing: f64,
) -> Result<(), PolysolidError> {
    let tolerance = TANGENT_TOLERANCE.min(closing);
    for joint in usize::from(!closed)..edges.len() {
        let before = (joint + edges.len() - 1) % edges.len();
        if is_line(&edges[before]) && is_line(&edges[joint]) {
            continue;
        }
        let incoming = tangent(&edges[before], true);
        let outgoing = tangent(&edges[joint], false);
        let angle = cross(incoming, outgoing)
            .atan2(dot(incoming, outgoing))
            .abs();
        if angle > tolerance {
            return Err(parameter(format!(
                "polysolid path joint {joint} between segment {before} and segment {joint} is not tangent"
            )));
        }
    }
    Ok(())
}

pub(super) fn check_short_legs(
    edges: &[ProfileEdge],
    closed: bool,
    width: f64,
) -> Result<(), PolysolidError> {
    let count = edges.len();
    for (index, edge) in edges.iter().enumerate() {
        let &ProfileEdge::Line { from, to } = edge else {
            continue;
        };
        let before = (closed || index > 0).then(|| (index + count - 1) % count);
        let after = (closed || index + 1 < count).then(|| (index + 1) % count);
        let meets_line = [before, after]
            .into_iter()
            .flatten()
            .any(|other| other != index && is_line(&edges[other]));
        if meets_line && distance(from, to) < width / 2.0 {
            return Err(parameter(format!(
                "polysolid path segment {index} is a leg shorter than half the width"
            )));
        }
    }
    Ok(())
}

pub(super) fn check_overlap(
    ring: &[ProfileEdge],
    segments: &[usize],
    g: f64,
) -> Result<(), PolysolidError> {
    if !profile_self_intersects(ring, true, g) {
        return Ok(());
    }
    let count = ring.len();
    let pair = (0..count)
        .flat_map(|first| ((first + 1)..count).map(move |second| (first, second)))
        .find(|&(first, second)| {
            let neighbours = if second == first + 1 {
                Some([first, second])
            } else if first == 0 && second == count - 1 {
                Some([second, first])
            } else {
                None
            };
            match neighbours {
                Some([a, b]) => {
                    profile_self_intersects(&[ring[a].clone(), ring[b].clone()], false, g)
                }
                None => !profile_edge_intersections(&ring[first], &ring[second], g).is_empty(),
            }
        });
    Err(overlap(
        pair.map(|(first, second)| [segments[first], segments[second]]),
    ))
}

pub(super) fn check_rings_apart(
    (outer, outer_segments): (&[ProfileEdge], &[usize]),
    (hole, hole_segments): (&[ProfileEdge], &[usize]),
    g: f64,
) -> Result<(), PolysolidError> {
    for (first, outer_edge) in outer.iter().enumerate() {
        for (second, hole_edge) in hole.iter().enumerate() {
            if !profile_edge_intersections(outer_edge, hole_edge, g).is_empty() {
                return Err(overlap(Some([
                    outer_segments[first],
                    hole_segments[second],
                ])));
            }
        }
    }
    Ok(())
}

pub(super) fn check_leg_kept(
    piece: &ProfileEdge,
    edge: &ProfileEdge,
    pair: [usize; 2],
    g: f64,
) -> Result<(), PolysolidError> {
    let (&ProfileEdge::Line { from, to }, true) = (piece, is_line(edge)) else {
        return Ok(());
    };
    if dot([to[0] - from[0], to[1] - from[1]], tangent(edge, false)) <= 4.0 * g {
        return Err(overlap(Some(pair)));
    }
    Ok(())
}

fn overlap(segments: Option<[usize; 2]>) -> PolysolidError {
    parameter(match segments {
        Some([a, b]) => format!(
            "polysolid wall overlaps itself between segment {} and segment {}",
            a.min(b),
            a.max(b)
        ),
        None => "polysolid wall overlaps itself".into(),
    })
}

pub(super) fn tangent(edge: &ProfileEdge, at_end: bool) -> [f64; 2] {
    match *edge {
        ProfileEdge::Line { from, to } => {
            let length = (to[0] - from[0]).hypot(to[1] - from[1]);
            [(to[0] - from[0]) / length, (to[1] - from[1]) / length]
        }
        ProfileEdge::Arc {
            start_angle,
            sweep_angle,
            ..
        } => {
            let angle = start_angle + if at_end { sweep_angle } else { 0.0 };
            let sense = sweep_angle.signum();
            [-angle.sin() * sense, angle.cos() * sense]
        }
    }
}

pub(super) fn is_line(edge: &ProfileEdge) -> bool {
    matches!(edge, ProfileEdge::Line { .. })
}

pub(super) fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

pub(super) fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
