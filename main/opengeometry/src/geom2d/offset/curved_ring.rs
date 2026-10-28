use crate::brep::GeometryError;
use crate::geom2d::boolean2d::{distance_to_segment, PlanarBooleanOp};
use crate::geom2d::curved_boolean2d::{
    boolean_curved_regions, reversed_ring, CurveEdge2, CurveRegion2,
};
use crate::geom2d::poly2d::Pt2;
use std::f64::consts::PI;

pub(crate) fn inset_curved_region(
    region: &CurveRegion2,
    distance: f64,
    eps: f64,
) -> Result<Vec<CurveRegion2>, GeometryError> {
    if !distance.is_finite() || distance <= 4.0 * eps {
        return Err(GeometryError::UnresolvedIntersection(
            "curved inset distance is below resolution".into(),
        ));
    }
    let at = centre(region);
    let region = moved(region, [-at[0], -at[1]]);
    let base = CurveRegion2 {
        outer: oriented_ring(&region.outer, false),
        holes: region
            .holes
            .iter()
            .map(|hole| oriented_ring(hole, true))
            .collect(),
    };
    let cutters: Vec<CurveRegion2> = std::iter::once(&base.outer)
        .chain(&base.holes)
        .flatten()
        .flat_map(|edge| bands(edge, distance, eps))
        .collect();
    let regions = boolean_curved_regions(&[base], &cutters, PlanarBooleanOp::Subtraction, eps)?;
    Ok(regions
        .into_iter()
        .filter(|region| !sliver(&region.outer, eps))
        .map(|region| {
            let sealed_region = CurveRegion2 {
                outer: sealed(merged(region.outer, eps), eps),
                holes: region
                    .holes
                    .into_iter()
                    .map(|hole| sealed(merged(hole, eps), eps))
                    .collect(),
            };
            moved(&sealed_region, at)
        })
        .collect())
}

fn samples(region: &CurveRegion2) -> impl Iterator<Item = Pt2> + '_ {
    std::iter::once(&region.outer)
        .chain(&region.holes)
        .flatten()
        .flat_map(|edge| [edge.point(0.0), edge.point(0.5), edge.point(1.0)])
}

fn centre(region: &CurveRegion2) -> [f64; 2] {
    let (low, high) = samples(region).fold(
        ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]),
        |(low, high), point| {
            (
                [low[0].min(point.x), low[1].min(point.z)],
                [high[0].max(point.x), high[1].max(point.z)],
            )
        },
    );
    [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0]
}

fn moved(region: &CurveRegion2, by: [f64; 2]) -> CurveRegion2 {
    let shift = |point: &mut [f64; 2]| *point = [point[0] + by[0], point[1] + by[1]];
    let ring = |ring: &Vec<CurveEdge2>| {
        let mut ring = ring.clone();
        for edge in &mut ring {
            match edge {
                CurveEdge2::Line { from, to } => {
                    shift(from);
                    shift(to);
                }
                CurveEdge2::Arc { center, .. } => shift(center),
            }
        }
        ring
    };
    CurveRegion2 {
        outer: ring(&region.outer),
        holes: region.holes.iter().map(ring).collect(),
    }
}

fn bands(edge: &CurveEdge2, distance: f64, eps: f64) -> [CurveRegion2; 2] {
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

fn disk(center: [f64; 2], radius: f64) -> CurveRegion2 {
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

fn sliver(ring: &[CurveEdge2], eps: f64) -> bool {
    let perimeter: f64 = ring.iter().map(CurveEdge2::length).sum();
    twice_area(ring).abs() < eps * perimeter
}

fn merged(ring: Vec<CurveEdge2>, eps: f64) -> Vec<CurveEdge2> {
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

fn sealed(mut ring: Vec<CurveEdge2>, eps: f64) -> Vec<CurveEdge2> {
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

fn angle(center: [f64; 2], point: Pt2) -> f64 {
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

pub(crate) fn inset_tolerance(section: &CurveRegion2, intersection: f64) -> f64 {
    let at = centre(section);
    let extent = samples(section).fold(0.0, |extent: f64, point| {
        extent
            .max((point.x - at[0]).abs())
            .max((point.z - at[1]).abs())
    });
    (1e-7 * extent).max(intersection)
}

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
