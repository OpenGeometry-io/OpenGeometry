use crate::geom2d::curved_boolean2d::CurveEdge2;
use crate::geom2d::poly2d::Pt2;
use std::f64::consts::PI;

pub(crate) fn offset_from(
    edge: &CurveEdge2,
    source: &CurveEdge2,
    thickness: f64,
    tolerance: f64,
) -> bool {
    let near = |a: Pt2, b: [f64; 2]| (a.x - b[0]).hypot(a.z - b[1]) <= tolerance;
    match (edge, source) {
        (
            CurveEdge2::Line {
                from: start,
                to: end,
            },
            CurveEdge2::Line { from, to },
        ) => {
            let length = (to[0] - from[0]).hypot(to[1] - from[1]);
            let along = [(to[0] - from[0]) / length, (to[1] - from[1]) / length];
            let local = |point: [f64; 2]| {
                let offset = [point[0] - from[0], point[1] - from[1]];
                (
                    along[0] * offset[0] + along[1] * offset[1],
                    (along[0] * offset[1] - along[1] * offset[0]).abs(),
                )
            };
            let (first, first_distance) = local(*start);
            let (last, last_distance) = local(*end);
            (first_distance - thickness).abs() <= tolerance
                && (last_distance - thickness).abs() <= tolerance
                && first.max(last).min(length) - first.min(last).max(0.0) > tolerance
        }
        (CurveEdge2::Arc { center, radius, .. }, _) => {
            let round = (radius - thickness).abs() <= tolerance
                && (near(source.point(0.0), *center) || near(source.point(1.0), *center));
            round || concentric(edge, source, thickness, tolerance)
        }
        _ => false,
    }
}

fn concentric(edge: &CurveEdge2, source: &CurveEdge2, thickness: f64, tolerance: f64) -> bool {
    let (
        CurveEdge2::Arc {
            center,
            radius,
            start_angle: edge_start,
            sweep_angle: edge_sweep,
        },
        CurveEdge2::Arc {
            center: at,
            radius: size,
            start_angle,
            sweep_angle,
        },
    ) = (edge, source)
    else {
        return false;
    };
    let sign = sweep_angle.signum();
    let first = if edge_sweep.signum() == sign {
        *edge_start
    } else {
        edge_start + edge_sweep
    };
    let lead = ((first - start_angle) * sign).rem_euclid(2.0 * PI);
    (at[0] - center[0]).hypot(at[1] - center[1]) <= tolerance
        && ((radius - (size - thickness)).abs() <= tolerance
            || (radius - (size + thickness)).abs() <= tolerance)
        && angular_overlap(lead, edge_sweep.abs(), sweep_angle.abs()) * radius > tolerance
}

fn angular_overlap(lead: f64, span: f64, source_span: f64) -> f64 {
    [lead, lead - 2.0 * PI]
        .iter()
        .map(|start| ((start + span).min(source_span) - start.max(0.0)).max(0.0))
        .sum()
}
