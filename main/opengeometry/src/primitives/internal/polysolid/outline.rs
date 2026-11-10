use super::super::arc_edged_extrusion::arc_edged_extrusion_with_holes;
use super::checks::{
    check_leg_kept, check_overlap, check_path, check_rings_apart, check_short_legs,
    check_tangent_joints, cross, dot, is_line, tangent,
};
use super::path::{PolysolidError, PolysolidJustify, PolysolidPath, MITER_LIMIT};
use crate::brep::{Accuracy, BrepEnvelope, Frame3};
use crate::primitives::profile::{profile_area_twice, ProfileEdge};
use crate::primitives::profile_wire::profile_segment_edges;

struct Side {
    edges: Vec<ProfileEdge>,
    segments: Vec<usize>,
}

pub(super) fn arc_polysolid(
    id: String,
    frame: Frame3,
    path: &PolysolidPath,
    justify: PolysolidJustify,
    [width, height]: [f64; 2],
    accuracy: Accuracy,
) -> Result<BrepEnvelope, PolysolidError> {
    let g = accuracy.geometric;
    let edges = profile_segment_edges(path.start, &path.segments, path.closed);
    check_path(path, &edges, g)?;
    let [left, right] = match justify {
        PolysolidJustify::Centre => [width / 2.0, -width / 2.0],
        PolysolidJustify::Left => [width, 0.0],
        PolysolidJustify::Right => [0.0, -width],
    };
    check_tangent_joints(&edges, path.closed, g / (4.0 * left.abs().max(right.abs())))?;
    check_short_legs(&edges, path.closed, width)?;
    let side_a = offset_side(&edges, path.closed, left, g)?;
    let side_b = offset_side(&edges, path.closed, right, g)?;
    let rings = if !path.closed {
        vec![open_outline(side_a, side_b)]
    } else if profile_area_twice(&side_a.edges).abs() >= profile_area_twice(&side_b.edges).abs() {
        vec![side_a, side_b]
    } else {
        vec![side_b, side_a]
    };
    for ring in &rings {
        check_overlap(&ring.edges, &ring.segments, g)?;
    }
    if let [outer, hole] = rings.as_slice() {
        check_rings_apart(
            (&outer.edges, &outer.segments),
            (&hole.edges, &hole.segments),
            g,
        )?;
    }
    let mut rings = rings.into_iter().map(|ring| ring.edges);
    let outer = rings.next().unwrap_or_default();
    arc_edged_extrusion_with_holes(id, frame, outer, rings.collect(), height, accuracy)
        .map_err(PolysolidError::Geometry)
}

fn offset_side(
    edges: &[ProfileEdge],
    closed: bool,
    distance: f64,
    g: f64,
) -> Result<Side, PolysolidError> {
    let mut pieces = edges
        .iter()
        .enumerate()
        .map(|(index, edge)| offset_piece(edge, index, distance, g))
        .collect::<Result<Vec<_>, _>>()?;
    let mut side = Side {
        edges: Vec::with_capacity(2 * pieces.len()),
        segments: Vec::with_capacity(2 * pieces.len()),
    };
    let mut bevels = vec![None; pieces.len()];
    for joint in usize::from(!closed)..pieces.len() {
        let before = (joint + pieces.len() - 1) % pieces.len();
        bevels[joint] = join(&mut pieces, [before, joint], edges, distance);
    }
    let count = pieces.len();
    for (index, piece) in pieces.iter().enumerate() {
        let next = (index + 1) % count;
        let pair = if (closed || index + 1 < count) && is_line(&edges[next]) {
            [index, next]
        } else {
            [(index + count - 1) % count, index]
        };
        check_leg_kept(piece, &edges[index], pair, g)?;
    }
    for (index, (bevel, piece)) in bevels.into_iter().zip(pieces).enumerate() {
        if let Some(bevel) = bevel {
            side.edges.push(bevel);
            side.segments.push(index);
        }
        side.edges.push(piece);
        side.segments.push(index);
    }
    Ok(side)
}

fn offset_piece(
    edge: &ProfileEdge,
    index: usize,
    distance: f64,
    g: f64,
) -> Result<ProfileEdge, PolysolidError> {
    match *edge {
        ProfileEdge::Line { from, to } => {
            let normal = left_normal(tangent(edge, false));
            Ok(ProfileEdge::Line {
                from: along(from, normal, distance),
                to: along(to, normal, distance),
            })
        }
        ProfileEdge::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => {
            let offset = radius - distance * sweep_angle.signum();
            if offset <= 4.0 * g {
                return Err(PolysolidError::Parameter(format!(
                    "polysolid path segment {index} is an arc whose offset radius is not above four times the tolerance"
                )));
            }
            Ok(ProfileEdge::Arc {
                center,
                radius: offset,
                start_angle,
                sweep_angle,
            })
        }
    }
}

fn join(
    pieces: &mut [ProfileEdge],
    [before, joint]: [usize; 2],
    path: &[ProfileEdge],
    distance: f64,
) -> Option<ProfileEdge> {
    let (end, start) = (pieces[before].endpoints().1, pieces[joint].endpoints().0);
    match (is_line(&pieces[before]), is_line(&pieces[joint])) {
        (true, false) => set_end(&mut pieces[before], start),
        (false, true) => set_start(&mut pieces[joint], end),
        (false, false) => {}
        (true, true) => {
            let incoming = tangent(&path[before], true);
            let outgoing = tangent(&path[joint], false);
            let turn = cross(incoming, outgoing);
            let deflection = dot(incoming, outgoing).clamp(-1.0, 1.0).acos();
            if distance * turn < 0.0 && 1.0 / (deflection * 0.5).cos() > MITER_LIMIT {
                return Some(ProfileEdge::Line {
                    from: end,
                    to: start,
                });
            }
            let (normals, at) = (
                [left_normal(incoming), left_normal(outgoing)],
                path[joint].endpoints().0,
            );
            let scale = distance / (1.0 + dot(normals[0], normals[1]));
            let corner = along(at, add(normals[0], normals[1]), scale);
            set_end(&mut pieces[before], corner);
            set_start(&mut pieces[joint], corner);
        }
    }
    None
}

fn open_outline(side_a: Side, side_b: Side) -> Side {
    let a_ends = (
        side_a.edges[0].endpoints().0,
        side_a.edges[side_a.edges.len() - 1].endpoints().1,
    );
    let b_ends = (
        side_b.edges[0].endpoints().0,
        side_b.edges[side_b.edges.len() - 1].endpoints().1,
    );
    let last = side_a.segments[side_a.segments.len() - 1];
    let mut outline = side_a;
    outline.edges.push(ProfileEdge::Line {
        from: a_ends.1,
        to: b_ends.1,
    });
    outline.segments.push(last);
    outline
        .edges
        .extend(side_b.edges.iter().rev().map(ProfileEdge::reversed));
    outline.segments.extend(side_b.segments.iter().rev());
    outline.edges.push(ProfileEdge::Line {
        from: b_ends.0,
        to: a_ends.0,
    });
    outline.segments.push(0);
    outline
}

fn set_start(edge: &mut ProfileEdge, point: [f64; 2]) {
    if let ProfileEdge::Line { from, .. } = edge {
        *from = point;
    }
}

fn set_end(edge: &mut ProfileEdge, point: [f64; 2]) {
    if let ProfileEdge::Line { to, .. } = edge {
        *to = point;
    }
}

fn left_normal(direction: [f64; 2]) -> [f64; 2] {
    [-direction[1], direction[0]]
}

fn along(point: [f64; 2], direction: [f64; 2], distance: f64) -> [f64; 2] {
    [
        point[0] + direction[0] * distance,
        point[1] + direction[1] * distance,
    ]
}

fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
