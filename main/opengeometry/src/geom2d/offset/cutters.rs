use super::tidy::oriented_ring;
use crate::geom2d::curved_boolean2d::{CurveEdge2, CurveRegion2};
use std::f64::consts::PI;

pub(super) fn bands(edge: &CurveEdge2, distance: f64, eps: f64) -> [CurveRegion2; 2] {
    let start = edge.point(0.0);
    let band = match edge {
        CurveEdge2::Line { from, to } => rectangle(*from, *to, distance),
        CurveEdge2::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => sector(
            *center,
            [*radius - distance, *radius + distance],
            *start_angle,
            *sweep_angle,
            eps,
        ),
    };
    [band, disk([start.x, start.z], distance)]
}

fn rectangle(from: [f64; 2], to: [f64; 2], distance: f64) -> CurveRegion2 {
    let along = (to[0] - from[0]).hypot(to[1] - from[1]);
    let offset = [
        -(to[1] - from[1]) / along * distance,
        (to[0] - from[0]) / along * distance,
    ];
    let side =
        |point: [f64; 2], sign: f64| [point[0] + sign * offset[0], point[1] + sign * offset[1]];
    let corners = [
        side(from, -1.0),
        side(to, -1.0),
        side(to, 1.0),
        side(from, 1.0),
    ];
    let ring = (0..4)
        .map(|index| CurveEdge2::Line {
            from: corners[index],
            to: corners[(index + 1) % 4],
        })
        .collect::<Vec<_>>();
    region(oriented_ring(&ring, false))
}

fn sector(
    center: [f64; 2],
    radii: [f64; 2],
    start_angle: f64,
    sweep_angle: f64,
    eps: f64,
) -> CurveRegion2 {
    let [inner, outer] = radii;
    let arc = |radius: f64, start_angle: f64, sweep_angle: f64| CurveEdge2::Arc {
        center,
        radius,
        start_angle,
        sweep_angle,
    };
    let outer_arc = arc(outer, start_angle, sweep_angle);
    let ring = if inner * sweep_angle.abs() > 4.0 * eps {
        let inner_arc = arc(inner, start_angle + sweep_angle, -sweep_angle);
        vec![
            outer_arc.clone(),
            joining(&outer_arc, &inner_arc),
            inner_arc.clone(),
            joining(&inner_arc, &outer_arc),
        ]
    } else {
        let apex = CurveEdge2::Line {
            from: center,
            to: center,
        };
        vec![
            joining(&apex, &outer_arc),
            outer_arc.clone(),
            joining(&outer_arc, &apex),
        ]
    };
    region(oriented_ring(&ring, false))
}

fn joining(before: &CurveEdge2, after: &CurveEdge2) -> CurveEdge2 {
    let from = before.point(1.0);
    let to = after.point(0.0);
    CurveEdge2::Line {
        from: [from.x, from.z],
        to: [to.x, to.z],
    }
}

pub(super) fn disk(center: [f64; 2], radius: f64) -> CurveRegion2 {
    let half = |start_angle: f64| CurveEdge2::Arc {
        center,
        radius,
        start_angle,
        sweep_angle: -PI,
    };
    region(vec![half(PI), half(0.0)])
}

fn region(outer: Vec<CurveEdge2>) -> CurveRegion2 {
    CurveRegion2 {
        outer,
        holes: Vec::new(),
    }
}
