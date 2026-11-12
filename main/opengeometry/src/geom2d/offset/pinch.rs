use super::tidy::angle;
use crate::geom2d::boolean2d::distance_to_segment;
use crate::geom2d::curved_boolean2d::{CurveEdge2, CurveRegion2};
use crate::geom2d::poly2d::Pt2;
use std::f64::consts::PI;

pub(crate) fn pinched(region: &CurveRegion2, gap: f64) -> bool {
    let rings = std::iter::once(&region.outer)
        .chain(&region.holes)
        .collect::<Vec<_>>();
    rings.iter().enumerate().any(|(index, ring)| {
        rings[index..].iter().enumerate().any(|(offset, other)| {
            ring.iter().enumerate().any(|(first, edge)| {
                other.iter().enumerate().any(|(second, next)| {
                    let apart =
                        offset > 0 || (second > first + 1 && second + 1 < first + ring.len());
                    apart && edge_gap(edge, next) < gap
                })
            })
        })
    })
}

fn edge_gap(edge: &CurveEdge2, other: &CurveEdge2) -> f64 {
    nearest_candidates(edge, other)
        .into_iter()
        .map(|point| point_gap(other, point))
        .chain(
            nearest_candidates(other, edge)
                .into_iter()
                .map(|point| point_gap(edge, point)),
        )
        .fold(f64::INFINITY, f64::min)
}

fn nearest_candidates(edge: &CurveEdge2, other: &CurveEdge2) -> Vec<Pt2> {
    let mut points = vec![edge.point(0.0), edge.point(1.0)];
    if let CurveEdge2::Arc {
        center,
        radius,
        start_angle,
        sweep_angle,
    } = edge
    {
        let toward = match other {
            CurveEdge2::Line { from, to } => (to[1] - from[1]).atan2(to[0] - from[0]) + PI / 2.0,
            CurveEdge2::Arc { center: at, .. } => (at[1] - center[1]).atan2(at[0] - center[0]),
        };
        points.extend(
            [toward, toward + PI]
                .into_iter()
                .filter(|angle| spans(*start_angle, *sweep_angle, *angle))
                .map(|angle| {
                    Pt2::new(
                        center[0] + radius * angle.cos(),
                        center[1] + radius * angle.sin(),
                    )
                }),
        );
    }
    points
}

fn point_gap(edge: &CurveEdge2, point: Pt2) -> f64 {
    match edge {
        CurveEdge2::Line { from, to } => {
            distance_to_segment(point, Pt2::new(from[0], from[1]), Pt2::new(to[0], to[1]))
        }
        CurveEdge2::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } if spans(*start_angle, *sweep_angle, angle(*center, point)) => {
            ((point.x - center[0]).hypot(point.z - center[1]) - radius).abs()
        }
        CurveEdge2::Arc { .. } => [edge.point(0.0), edge.point(1.0)]
            .iter()
            .map(|end| (end.x - point.x).hypot(end.z - point.z))
            .fold(f64::INFINITY, f64::min),
    }
}

fn spans(start_angle: f64, sweep_angle: f64, angle: f64) -> bool {
    ((angle - start_angle) * sweep_angle.signum()).rem_euclid(2.0 * PI) <= sweep_angle.abs()
}
