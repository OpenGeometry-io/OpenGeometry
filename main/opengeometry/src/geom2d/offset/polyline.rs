use super::ring::OffsetError;
use crate::geom2d::boolean2d::PlanarBooleanOp;
use crate::geom2d::curved_boolean2d::{boolean_curved_regions, CurveEdge2, CurveRegion2};
use crate::geom2d::poly2d::{cross2, dot2, signed_area2, Pt2};

#[derive(Clone, Copy)]
pub(crate) struct OffsetOptions {
    pub(crate) bevel: bool,
    pub(crate) acute_threshold_degrees: f64,
}

pub(crate) struct OffsetPath {
    pub(crate) edges: Vec<CurveEdge2>,
    pub(crate) bevelled: Vec<usize>,
    pub(crate) closed: bool,
}

struct Segments {
    points: Vec<Pt2>,
    directions: Vec<Pt2>,
    normals: Vec<Pt2>,
}

pub(crate) fn offset_polyline_regions(
    path: &[Pt2],
    width: f64,
    closed: bool,
    miter_limit: f64,
    eps: f64,
) -> Result<Vec<CurveRegion2>, OffsetError> {
    if !width.is_finite() {
        return Err(OffsetError::InvalidParameter(format!(
            "stroke width {width} is not a finite number"
        )));
    }
    if width <= 4.0 * eps || path.len() < 2 {
        return Ok(Vec::new());
    }
    let half = width * 0.5;
    let (points, closed) = closed_points(&simplified(path, half, closed, eps), closed, eps);
    let Some(segments) = segments(points, closed, eps) else {
        return Ok(Vec::new());
    };
    let count = segments.directions.len();
    let mut pieces: Vec<CurveRegion2> = (0..count)
        .map(|index| {
            let (a, b) = (
                segments.points[index],
                segments.points[(index + 1) % segments.points.len()],
            );
            let n = segments.normals[index];
            polygon(vec![
                along(a, n, half),
                along(b, n, half),
                along(b, n, -half),
                along(a, n, -half),
            ])
        })
        .collect();
    let joints = if closed { 0..count } else { 1..count };
    for vertex in joints {
        let previous = (vertex + count - 1) % count;
        if let Some(wedge) = wedge(&segments, vertex, previous, half, miter_limit, eps) {
            pieces.push(wedge);
        }
    }
    boolean_curved_regions(&pieces, &[], PlanarBooleanOp::Union, eps).map_err(OffsetError::Clip)
}

fn wedge(
    segments: &Segments,
    vertex: usize,
    previous: usize,
    half: f64,
    miter_limit: f64,
    eps: f64,
) -> Option<CurveRegion2> {
    let at = segments.points[vertex];
    let (incoming, outgoing) = (segments.directions[previous], segments.directions[vertex]);
    let turn = cross2(incoming, outgoing);
    let side = if turn > 0.0 { -1.0 } else { 1.0 };
    let first = along(at, segments.normals[previous], side * half);
    let second = along(at, segments.normals[vertex], side * half);
    if length(first, second) <= 4.0 * eps {
        return None;
    }
    let deflection = dot2(incoming, outgoing).clamp(-1.0, 1.0).acos();
    if 1.0 / (deflection * 0.5).cos() > miter_limit {
        let height = cross2(
            Pt2::new(first.x - at.x, first.z - at.z),
            Pt2::new(second.x - at.x, second.z - at.z),
        );
        if height.abs() <= eps * length(first, second) {
            return None;
        }
        return Some(polygon(vec![at, first, second]));
    }
    let corner = meet(first, incoming, second, outgoing)?;
    Some(polygon(vec![at, first, corner, second]))
}

pub(crate) fn offset_path(
    path: &[Pt2],
    distance: f64,
    force_closed: Option<bool>,
    options: OffsetOptions,
    eps: f64,
) -> OffsetPath {
    let (points, closed) = sanitized(path, force_closed, eps);
    let mut result = OffsetPath {
        edges: Vec::new(),
        bevelled: Vec::new(),
        closed,
    };
    if points.len() < 2 || (closed && points.len() < 3) {
        return result;
    }
    if distance.abs() <= eps {
        result.edges = chain(&points, closed);
        return result;
    }
    let Some(segments) = segments(points, closed, eps) else {
        return result;
    };
    let count = segments.points.len();
    let mut offset: Vec<Pt2> = Vec::with_capacity(count + 2);
    if !closed {
        push_unique(
            &mut offset,
            along(segments.points[0], segments.normals[0], distance),
            eps,
        );
    }
    let corners = if closed { 0..count } else { 1..count - 1 };
    for vertex in corners {
        let bevelled = corner(
            &segments,
            vertex,
            closed,
            distance,
            options,
            &mut offset,
            eps,
        );
        if bevelled {
            result.bevelled.push(vertex);
        }
    }
    if !closed {
        let last = segments.normals.len() - 1;
        push_unique(
            &mut offset,
            along(segments.points[count - 1], segments.normals[last], distance),
            eps,
        );
    }
    result.edges = chain(&offset, closed);
    result
}

fn corner(
    segments: &Segments,
    vertex: usize,
    closed: bool,
    distance: f64,
    options: OffsetOptions,
    offset: &mut Vec<Pt2>,
    eps: f64,
) -> bool {
    let at = segments.points[vertex];
    let previous = (vertex + segments.directions.len() - 1) % segments.directions.len();
    let next = vertex % segments.directions.len();
    let (incoming, outgoing) = (segments.directions[previous], segments.directions[next]);
    let first = along(at, segments.normals[previous], distance);
    let second = along(at, segments.normals[next], distance);
    let dot = dot2(incoming, outgoing).clamp(-1.0, 1.0);
    let interior = std::f64::consts::PI - dot.acos();
    let threshold = options
        .acute_threshold_degrees
        .clamp(1.0, 179.0)
        .to_radians();
    let turn = cross2(incoming, outgoing);
    if turn.abs() <= 1.0e-7 && dot > 0.9999 {
        push_unique(offset, first, eps);
        return false;
    }
    let outer = turn * distance > eps;
    let inner = turn * distance < -eps;
    let clipped = !closed && inner && interior <= threshold;
    let bevelled = options.bevel && outer && interior <= threshold;
    if clipped || bevelled {
        push_unique(offset, first, eps);
        push_unique(offset, second, eps);
        return bevelled;
    }
    match meet(first, incoming, second, outgoing) {
        Some(point) => push_unique(offset, point, eps),
        None if options.bevel && outer => {
            push_unique(offset, first, eps);
            push_unique(offset, second, eps);
            return true;
        }
        None => push_unique(
            offset,
            Pt2::new((first.x + second.x) * 0.5, (first.z + second.z) * 0.5),
            eps,
        ),
    }
    false
}

fn sanitized(path: &[Pt2], force_closed: Option<bool>, eps: f64) -> (Vec<Pt2>, bool) {
    let mut points: Vec<Pt2> = Vec::with_capacity(path.len());
    for &point in path {
        push_unique(&mut points, point, eps);
    }
    let mut closed = force_closed.unwrap_or(false);
    if force_closed.is_none() && points.len() >= 3 {
        closed = length(points[0], points[points.len() - 1]) <= eps;
    }
    if closed && points.len() >= 2 && length(points[0], points[points.len() - 1]) <= eps {
        points.pop();
    }
    (points, closed)
}

fn closed_points(points: &[Pt2], closed: bool, eps: f64) -> (Vec<Pt2>, bool) {
    let mut points = points.to_vec();
    if points.len() >= 2 && length(points[0], points[points.len() - 1]) < eps {
        points.pop();
        return (points, true);
    }
    (points, closed)
}

fn simplified(path: &[Pt2], shortest: f64, closed: bool, eps: f64) -> Vec<Pt2> {
    if path.len() < 3 {
        return path.to_vec();
    }
    let mut points = path.to_vec();
    let duplicate = closed && length(points[0], points[points.len() - 1]) < eps;
    if duplicate {
        points.pop();
    }
    let mut kept = vec![points[0]];
    for &point in &points[1..] {
        if kept
            .last()
            .is_some_and(|last| length(point, *last) >= shortest)
        {
            kept.push(point);
        }
    }
    let end = points[points.len() - 1];
    if !closed {
        let count = kept.len();
        let gap = length(end, kept[count - 1]);
        if gap > eps && gap < shortest && count >= 2 {
            kept[count - 1] = end;
        } else if gap > eps {
            kept.push(end);
        }
    } else if kept.len() > 3 && length(kept[0], kept[kept.len() - 1]) < shortest {
        kept.pop();
    }
    if duplicate {
        kept.push(kept[0]);
    }
    let fewest = if !closed {
        2
    } else if duplicate {
        4
    } else {
        3
    };
    if kept.len() < fewest {
        return path.to_vec();
    }
    kept
}

fn segments(points: Vec<Pt2>, closed: bool, eps: f64) -> Option<Segments> {
    if points.len() < 2 || (closed && points.len() < 3) {
        return None;
    }
    let count = if closed {
        points.len()
    } else {
        points.len() - 1
    };
    let mut directions = Vec::with_capacity(count);
    let mut normals = Vec::with_capacity(count);
    for index in 0..count {
        let (a, b) = (points[index], points[(index + 1) % points.len()]);
        let span = length(a, b);
        if span <= eps {
            return None;
        }
        directions.push(Pt2::new((b.x - a.x) / span, (b.z - a.z) / span));
        normals.push(Pt2::new(-(b.z - a.z) / span, (b.x - a.x) / span));
    }
    Some(Segments {
        points,
        directions,
        normals,
    })
}

fn polygon(mut points: Vec<Pt2>) -> CurveRegion2 {
    if signed_area2(&points) > 0.0 {
        points.reverse();
    }
    CurveRegion2 {
        outer: chain(&points, true),
        holes: Vec::new(),
    }
}

fn chain(points: &[Pt2], closed: bool) -> Vec<CurveEdge2> {
    let count = if closed {
        points.len()
    } else {
        points.len() - 1
    };
    (0..count)
        .map(|index| {
            let (a, b) = (points[index], points[(index + 1) % points.len()]);
            CurveEdge2::Line {
                from: [a.x, a.z],
                to: [b.x, b.z],
            }
        })
        .collect()
}

fn meet(a: Pt2, along_a: Pt2, b: Pt2, along_b: Pt2) -> Option<Pt2> {
    let denominator = cross2(along_a, along_b);
    if denominator.abs() <= 1.0e-9 {
        return None;
    }
    let t = cross2(Pt2::new(b.x - a.x, b.z - a.z), along_b) / denominator;
    Some(Pt2::new(a.x + t * along_a.x, a.z + t * along_a.z))
}

fn along(point: Pt2, normal: Pt2, distance: f64) -> Pt2 {
    Pt2::new(point.x + normal.x * distance, point.z + normal.z * distance)
}

fn push_unique(points: &mut Vec<Pt2>, point: Pt2, eps: f64) {
    if points.last().is_none_or(|last| length(*last, point) > eps) {
        points.push(point);
    }
}

fn length(a: Pt2, b: Pt2) -> f64 {
    (b.x - a.x).hypot(b.z - a.z)
}
