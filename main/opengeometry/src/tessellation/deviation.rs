use super::grid::count;
use crate::brep::{
    bounds_chord_deviation, point_segment_distance, BrepEnvelope, Curve, CurveGeometry,
    EdgeGeometry, GeometryError, PatchBounds, Surface,
};
use crate::math::{cross, norm, scale, sub, Interval, Point3};

fn intersection_chord_deviation(
    brep: &BrepEnvelope,
    definition: u32,
    range: Interval,
    chord_start: Point3,
    chord_end: Point3,
) -> Result<f64, GeometryError> {
    let definition = brep
        .geometry
        .intersections
        .get(definition as usize)
        .ok_or_else(|| GeometryError::InvalidGeometry("missing intersection definition".into()))?;
    let support_a = brep.geometry.surface(definition.surfaces[0])?;
    let support_b = brep.geometry.surface(definition.surfaces[1])?;
    let mut maximum = 0.0_f64;
    for (index, tube) in definition.uv_tubes.iter().enumerate() {
        let anchor_start = definition.anchors[index].parameter;
        let anchor_end = definition.anchors[index + 1].parameter;
        let lo = range.lo.max(anchor_start);
        let hi = range.hi.min(anchor_end);
        if lo > hi {
            continue;
        }
        let bounds_a = support_a.enclose([tube[0], tube[1]])?;
        let bounds_b = support_b.enclose([tube[2], tube[3]])?;
        let mut axes = [Interval::point(0.0)?; 3];
        for axis in 0..3 {
            axes[axis] = Interval::new(
                bounds_a.axes[axis].lo.max(bounds_b.axes[axis].lo),
                bounds_a.axes[axis].hi.min(bounds_b.axes[axis].hi),
            )
            .map_err(|_| GeometryError::InvalidGeometry("disjoint trace support boxes".into()))?;
        }
        let local_start = definition.evaluate(lo, &brep.geometry)?.point;
        let local_end = definition.evaluate(hi, &brep.geometry)?.point;
        let local_deviation = bounds_chord_deviation(PatchBounds { axes }, local_start, local_end);
        let guide_deviation = point_segment_distance(local_start, chord_start, chord_end)
            .max(point_segment_distance(local_end, chord_start, chord_end));
        maximum = maximum.max(local_deviation + guide_deviation);
    }
    Ok(maximum)
}

fn enclosed_intersection_count(
    brep: &BrepEnvelope,
    definition: u32,
    curve: u32,
    range: Interval,
    error: f64,
    max_segments: usize,
) -> Result<usize, GeometryError> {
    let evaluator = brep.geometry.curve(curve)?;
    let mut stack = vec![(range, 0usize)];
    let mut maximum_depth = 0usize;
    let mut visits = 0usize;
    while let Some((interval, depth)) = stack.pop() {
        visits = visits
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("curve tessellation visits".into()))?;
        if visits > max_segments.saturating_mul(2) {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation visits".into(),
            ));
        }
        let start = evaluator.point_at(interval.lo)?;
        let end = evaluator.point_at(interval.hi)?;
        if intersection_chord_deviation(brep, definition, interval, start, end)? <= error {
            maximum_depth = maximum_depth.max(depth);
            continue;
        }
        if depth >= usize::BITS as usize - 1 || (1usize << (depth + 1)) > max_segments {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation segments".into(),
            ));
        }
        let midpoint = interval.midpoint();
        stack.push((Interval::new(midpoint, interval.hi)?, depth + 1));
        stack.push((Interval::new(interval.lo, midpoint)?, depth + 1));
    }
    Ok(1usize << maximum_depth)
}

fn adaptive_intersection_count(
    brep: &BrepEnvelope,
    definition: u32,
    curve: u32,
    range: Interval,
    error: f64,
    max_segments: usize,
) -> Result<usize, GeometryError> {
    let evaluator = brep.geometry.curve(curve)?;
    brep.geometry
        .intersections
        .get(definition as usize)
        .ok_or_else(|| GeometryError::InvalidGeometry("missing intersection definition".into()))?;
    let closed = norm(sub(
        evaluator.point_at(range.lo)?,
        evaluator.point_at(range.hi)?,
    )) <= brep.accuracy.intersection;
    let mut count = if closed { 8 } else { 1 };
    loop {
        let mut achieved = 0.0_f64;
        for segment in 0..count {
            let lo = range.lo + range.width() * segment as f64 / count as f64;
            let hi = range.lo + range.width() * (segment + 1) as f64 / count as f64;
            let start = evaluator.point_at(lo)?;
            let end = evaluator.point_at(hi)?;
            let chord = sub(end, start);
            let length = norm(chord);
            if length == 0.0 {
                achieved = f64::INFINITY;
                break;
            }
            let direction = scale(chord, 1.0 / length);
            let mut tangent_turn = 0.0_f64;
            for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let parameter = lo + (hi - lo) * fraction;
                let point = evaluator.point_at(parameter)?;
                achieved = achieved.max(point_segment_distance(point, start, end));
                let tangent = evaluator.tangent_at(parameter)?;
                let tangent = scale(tangent, 1.0 / norm(tangent));
                tangent_turn = tangent_turn.max(norm(cross(tangent, direction)));
            }
            achieved = achieved.max(0.25 * length * tangent_turn);
        }
        if achieved <= error {
            return Ok(count);
        }
        count = count
            .checked_mul(2)
            .filter(|next| *next <= max_segments)
            .ok_or_else(|| GeometryError::LimitExceeded("curve tessellation segments".into()))?;
    }
}

fn bounded_intersection_count(
    brep: &BrepEnvelope,
    definition: u32,
    curve: u32,
    range: Interval,
    error: f64,
    max_segments: usize,
) -> Result<usize, GeometryError> {
    match enclosed_intersection_count(brep, definition, curve, range, error, max_segments) {
        Ok(count) => Ok(count),
        Err(GeometryError::LimitExceeded(_)) => {
            let count =
                adaptive_intersection_count(brep, definition, curve, range, error, max_segments)?;
            Ok(count)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn edge_count(
    brep: &BrepEnvelope,
    id: u32,
    error: f64,
    max_triangles: usize,
) -> Result<usize, GeometryError> {
    match brep.topology.edges[id as usize].geometry {
        EdgeGeometry::Collapsed { .. } => Ok(1),
        EdgeGeometry::Curve { curve, range } => match &brep.geometry.curves[curve as usize] {
            CurveGeometry::Line { .. } => Ok(1),
            CurveGeometry::Circle { radius, .. } => count(
                range.width() * (radius / (8.0 * error)).sqrt(),
                if range.width() >= std::f64::consts::TAU - 1e-12 {
                    3
                } else {
                    1
                },
                max_triangles,
            ),
            CurveGeometry::Ellipse { major_radius, .. } => count(
                range.width() * (major_radius / (8.0 * error)).sqrt(),
                if range.width() >= std::f64::consts::TAU - 1e-12 {
                    3
                } else {
                    1
                },
                max_triangles,
            ),
            CurveGeometry::Intersection { definition } => {
                bounded_intersection_count(brep, *definition, curve, range, error, max_triangles)
            }
        },
    }
}
