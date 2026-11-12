use crate::geom2d::curved_boolean2d::{reversed_ring, CurveEdge2};
use crate::geom2d::poly2d::Pt2;
use std::f64::consts::PI;

pub(crate) fn oriented_ring(ring: &[CurveEdge2], positive: bool) -> Vec<CurveEdge2> {
    if (twice_area(ring) > 0.0) == positive {
        ring.to_vec()
    } else {
        reversed_ring(ring)
    }
}

fn twice_area(ring: &[CurveEdge2]) -> f64 {
    ring.iter().map(CurveEdge2::twice_area).sum()
}

pub(super) fn sliver(ring: &[CurveEdge2], eps: f64) -> bool {
    let perimeter: f64 = ring.iter().map(CurveEdge2::length).sum();
    twice_area(ring).abs() < eps * perimeter
}

pub(super) fn merged(ring: Vec<CurveEdge2>, eps: f64) -> Vec<CurveEdge2> {
    let mut out: Vec<CurveEdge2> = Vec::with_capacity(ring.len());
    for edge in ring {
        let merge = out.last().and_then(|last| joined(last, &edge, eps));
        match (merge, out.last_mut()) {
            (Some(merge), Some(last)) => *last = merge,
            _ => out.push(edge),
        }
    }
    while out.len() > 2 {
        let Some(edge) = joined(&out[out.len() - 1], &out[0], eps) else {
            break;
        };
        out[0] = edge;
        out.pop();
    }
    out
}

pub(super) fn sealed(mut ring: Vec<CurveEdge2>, eps: f64) -> Vec<CurveEdge2> {
    for index in 0..ring.len() {
        let next = (index + 1) % ring.len();
        let end = ring[index].point(1.0);
        let start = ring[next].point(0.0);
        let reach = |a: Pt2, b: Pt2| (a.x - b.x).hypot(a.z - b.z);
        let joint = Some(junction(&ring[index], &ring[next], eps))
            .filter(|joint| reach(*joint, end).max(reach(*joint, start)) <= eps)
            .unwrap_or(end);
        set_end(&mut ring[index], joint);
        set_start(&mut ring[next], joint);
    }
    ring
}

fn junction(before: &CurveEdge2, after: &CurveEdge2, eps: f64) -> Pt2 {
    let line_arc = |from: [f64; 2], to: [f64; 2], center: [f64; 2], radius: f64, end: Pt2| {
        let along = [to[0] - from[0], to[1] - from[1]];
        let t = ((center[0] - from[0]) * along[0] + (center[1] - from[1]) * along[1])
            / (along[0] * along[0] + along[1] * along[1]);
        let foot = Pt2::new(from[0] + t * along[0], from[1] + t * along[1]);
        let reach = (foot.x - center[0]).hypot(foot.z - center[1]);
        if (reach - radius).abs() <= eps {
            foot
        } else {
            end
        }
    };
    match (before, after) {
        (CurveEdge2::Line { from, to }, CurveEdge2::Arc { center, radius, .. }) => {
            line_arc(*from, *to, *center, *radius, after.point(0.0))
        }
        (CurveEdge2::Arc { center, radius, .. }, CurveEdge2::Line { from, to }) => {
            line_arc(*from, *to, *center, *radius, before.point(1.0))
        }
        (
            CurveEdge2::Arc { center, radius, .. },
            CurveEdge2::Arc {
                center: other,
                radius: other_radius,
                ..
            },
        ) => circle_contact(*center, *radius, *other, *other_radius, eps)
            .unwrap_or_else(|| before.point(1.0)),
        _ => before.point(1.0),
    }
}

fn circle_contact(
    center: [f64; 2],
    radius: f64,
    other: [f64; 2],
    other_radius: f64,
    eps: f64,
) -> Option<Pt2> {
    let gap = (other[0] - center[0]).hypot(other[1] - center[1]);
    let sign = if (gap - radius - other_radius).abs() <= eps
        || (radius > other_radius && (gap - radius + other_radius).abs() <= eps)
    {
        1.0
    } else if (gap - other_radius + radius).abs() <= eps {
        -1.0
    } else {
        return None;
    };
    (gap > eps).then(|| {
        let scale = sign * radius / gap;
        Pt2::new(
            center[0] + scale * (other[0] - center[0]),
            center[1] + scale * (other[1] - center[1]),
        )
    })
}

fn set_end(edge: &mut CurveEdge2, point: Pt2) {
    match edge {
        CurveEdge2::Line { to, .. } => *to = [point.x, point.z],
        CurveEdge2::Arc {
            center,
            start_angle,
            sweep_angle,
            ..
        } => {
            *sweep_angle =
                nearest(angle(*center, point), *start_angle + *sweep_angle) - *start_angle
        }
    }
}

fn set_start(edge: &mut CurveEdge2, point: Pt2) {
    match edge {
        CurveEdge2::Line { from, .. } => *from = [point.x, point.z],
        CurveEdge2::Arc {
            center,
            start_angle,
            sweep_angle,
            ..
        } => {
            let start = nearest(angle(*center, point), *start_angle);
            *sweep_angle -= start - *start_angle;
            *start_angle = start;
        }
    }
}

pub(super) fn angle(center: [f64; 2], point: Pt2) -> f64 {
    (point.z - center[1]).atan2(point.x - center[0])
}

fn nearest(angle: f64, near: f64) -> f64 {
    angle + 2.0 * PI * ((near - angle) / (2.0 * PI)).round()
}

fn joined(first: &CurveEdge2, second: &CurveEdge2, eps: f64) -> Option<CurveEdge2> {
    match (first, second) {
        (CurveEdge2::Line { from, to }, CurveEdge2::Line { to: end, .. }) => {
            let along = [to[0] - from[0], to[1] - from[1]];
            let next = [end[0] - to[0], end[1] - to[1]];
            let length = along[0].hypot(along[1]);
            let off = (along[0] * next[1] - along[1] * next[0]).abs() / length;
            (off <= eps && along[0] * next[0] + along[1] * next[1] > 0.0).then_some(
                CurveEdge2::Line {
                    from: *from,
                    to: *end,
                },
            )
        }
        (
            CurveEdge2::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            },
            CurveEdge2::Arc {
                center: next_center,
                radius: next_radius,
                sweep_angle: next_sweep,
                ..
            },
        ) => {
            let same = (center[0] - next_center[0]).hypot(center[1] - next_center[1]) <= eps
                && (radius - next_radius).abs() <= eps
                && sweep_angle.signum() == next_sweep.signum()
                && (sweep_angle + next_sweep).abs() < 2.0 * PI - 1e-9;
            same.then(|| CurveEdge2::Arc {
                center: *center,
                radius: *radius,
                start_angle: *start_angle,
                sweep_angle: sweep_angle + next_sweep,
            })
        }
        _ => None,
    }
}
