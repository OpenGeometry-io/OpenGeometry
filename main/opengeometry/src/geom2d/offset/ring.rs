use crate::brep::GeometryError;
use crate::geom2d::boolean2d::{distance_to_segment, ring_is_degenerate, PlanarBooleanOp};
use crate::geom2d::curved_boolean2d::{boolean_curved_regions, CurveEdge2, CurveRegion2};
use crate::geom2d::poly2d::{cross2, self_intersects2, signed_area2, Pt2};
use std::f64::consts::PI;

pub(crate) struct OffsetHole {
    pub(crate) ring: Vec<Pt2>,
    pub(crate) distances: Vec<f64>,
}

#[derive(Debug)]
pub(crate) enum OffsetError {
    InvalidParameter(String),
    InvalidTopology(String),
    Clip(GeometryError),
}

struct PreparedRing {
    points: Vec<Pt2>,
    distances: Vec<f64>,
    disks: Vec<(Pt2, f64)>,
}

pub(crate) fn offset_ring_variable(
    ring: &[Pt2],
    distances: &[f64],
    holes: &[OffsetHole],
    eps: f64,
) -> Result<Vec<CurveRegion2>, OffsetError> {
    let Some(outer) = prepared(ring, distances, true, "ring", eps)? else {
        return Ok(Vec::new());
    };
    let mut inner = Vec::with_capacity(holes.len());
    for (index, hole) in holes.iter().enumerate() {
        let label = format!("holes[{index}]");
        let Some(prepared_hole) = prepared(&hole.ring, &hole.distances, false, &label, eps)? else {
            return Err(OffsetError::InvalidTopology(format!(
                "{label} is degenerate: a hole needs at least 3 non-collinear, non-self-intersecting points"
            )));
        };
        inner.push(prepared_hole);
    }
    let base = CurveRegion2 {
        outer: lines(&outer.points),
        holes: inner.iter().map(|hole| lines(&hole.points)).collect(),
    };
    let all: Vec<&PreparedRing> = std::iter::once(&outer).chain(&inner).collect();
    let cutters: Vec<CurveRegion2> = std::iter::once(&outer)
        .chain(&inner)
        .flat_map(|ring| {
            stadiums(ring, &all, eps).into_iter().chain(
                ring.disks
                    .iter()
                    .filter(|disk| disk.1 > 4.0 * eps)
                    .map(|&(centre, radius)| disk(centre, radius)),
            )
        })
        .collect();
    let regions = boolean_curved_regions(&[base], &cutters, PlanarBooleanOp::Subtraction, eps)
        .map_err(OffsetError::Clip)?;
    Ok(regions
        .into_iter()
        .filter(|region| !thin(&region.outer, eps))
        .collect())
}

fn prepared(
    ring: &[Pt2],
    distances: &[f64],
    clockwise: bool,
    label: &str,
    eps: f64,
) -> Result<Option<PreparedRing>, OffsetError> {
    let mut points = ring.to_vec();
    if points.len() > 1 && length(points[0], points[points.len() - 1]) <= eps {
        points.pop();
    }
    check_distances(label, points.len(), distances)?;
    let mut kept = merged(&points, distances, eps);
    if kept.points.len() < 3 || flat(&kept.points, eps) {
        return Ok(None);
    }
    if self_intersects2(&kept.points, eps) {
        return Err(OffsetError::InvalidTopology(format!(
            "{label} crosses itself"
        )));
    }
    if ring_is_degenerate(&kept.points, eps) {
        return Ok(None);
    }
    if (signed_area2(&kept.points) < 0.0) != clockwise {
        reverse(&mut kept);
    }
    Ok(Some(kept))
}

fn merged(points: &[Pt2], distances: &[f64], eps: f64) -> PreparedRing {
    let mut kept = PreparedRing {
        points: Vec::with_capacity(points.len()),
        distances: Vec::with_capacity(points.len()),
        disks: Vec::new(),
    };
    for (&point, &distance) in points.iter().zip(distances) {
        match (kept.points.last(), kept.distances.last_mut()) {
            (Some(&last), Some(slot)) if length(last, point) <= 4.0 * eps => {
                kept.disks
                    .push((middle(last, point), *slot + length(last, point) * 0.5));
                *slot = distance;
            }
            _ => {
                kept.points.push(point);
                kept.distances.push(distance);
            }
        }
    }
    while kept.points.len() > 1
        && length(kept.points[0], kept.points[kept.points.len() - 1]) <= 4.0 * eps
    {
        if let (Some(last), Some(distance)) = (kept.points.pop(), kept.distances.pop()) {
            kept.disks.push((
                middle(last, kept.points[0]),
                distance + length(last, kept.points[0]) * 0.5,
            ));
        }
    }
    kept
}

fn flat(points: &[Pt2], eps: f64) -> bool {
    let start = points[0];
    let end = points
        .iter()
        .copied()
        .max_by(|a, b| length(start, *a).total_cmp(&length(start, *b)))
        .unwrap_or(start);
    let span = length(start, end);
    let along = Pt2::new(end.x - start.x, end.z - start.z);
    span <= eps
        || points.iter().all(|point| {
            cross2(along, Pt2::new(point.x - start.x, point.z - start.z)).abs() <= eps * span
        })
}

fn check_distances(label: &str, edges: usize, distances: &[f64]) -> Result<(), OffsetError> {
    if distances.len() != edges {
        return Err(OffsetError::InvalidParameter(format!(
            "Expected one distance per {label} edge: the ring has {edges} edges but {} distances were given",
            distances.len()
        )));
    }
    if let Some((index, distance)) = distances
        .iter()
        .enumerate()
        .find(|(_, distance)| !distance.is_finite() || **distance < 0.0)
    {
        return Err(OffsetError::InvalidParameter(format!(
            "{label} distances[{index}] = {distance} is invalid: every per-edge distance must be a finite value >= 0"
        )));
    }
    Ok(())
}

fn reverse(ring: &mut PreparedRing) {
    let count = ring.points.len();
    ring.points.reverse();
    let old = ring.distances.clone();
    for (index, slot) in ring.distances.iter_mut().enumerate() {
        *slot = old[(2 * count - 2 - index) % count];
    }
}

fn lines(points: &[Pt2]) -> Vec<CurveEdge2> {
    (0..points.len())
        .map(|index| CurveEdge2::Line {
            from: [points[index].x, points[index].z],
            to: [
                points[(index + 1) % points.len()].x,
                points[(index + 1) % points.len()].z,
            ],
        })
        .collect()
}

fn stadiums(ring: &PreparedRing, all: &[&PreparedRing], eps: f64) -> Vec<CurveRegion2> {
    let count = ring.points.len();
    (0..count)
        .filter(|&index| ring.distances[index] > 4.0 * eps)
        .map(|index| {
            let next = (index + 1) % count;
            let caps = [
                cap_needed(ring, index, index, (index + count - 1) % count, all),
                cap_needed(ring, next, index, next, all),
            ];
            stadium(
                ring.points[index],
                ring.points[next],
                ring.distances[index],
                caps,
            )
        })
        .collect()
}

fn cap_needed(
    ring: &PreparedRing,
    vertex: usize,
    edge: usize,
    neighbour: usize,
    all: &[&PreparedRing],
) -> bool {
    let count = ring.points.len();
    let at = ring.points[vertex];
    let before = ring.points[(vertex + count - 1) % count];
    let after = ring.points[(vertex + 1) % count];
    let turn = cross2(
        Pt2::new(at.x - before.x, at.z - before.z),
        Pt2::new(after.x - at.x, after.z - at.z),
    );
    let reach = ring.distances[edge];
    turn > 0.0 || reach > ring.distances[neighbour] || crowded(ring, vertex, reach, all)
}

fn crowded(ring: &PreparedRing, vertex: usize, reach: f64, all: &[&PreparedRing]) -> bool {
    let at = ring.points[vertex];
    all.iter().any(|other| {
        let count = other.points.len();
        (0..count).any(|index| {
            let own =
                std::ptr::eq(*other, ring) && (index == vertex || (index + 1) % count == vertex);
            !own && distance_to_segment(at, other.points[index], other.points[(index + 1) % count])
                < reach
        })
    })
}

fn stadium(a: Pt2, b: Pt2, distance: f64, caps: [bool; 2]) -> CurveRegion2 {
    let along = length(a, b);
    let normal = Pt2::new(-(b.z - a.z) / along, (b.x - a.x) / along);
    let side = |point: Pt2, sign: f64| {
        [
            point.x + sign * distance * normal.x,
            point.z + sign * distance * normal.z,
        ]
    };
    let facing = normal.z.atan2(normal.x);
    let cap = |point: Pt2, start_angle: f64, sign: f64, round: bool| {
        if round {
            CurveEdge2::Arc {
                center: [point.x, point.z],
                radius: distance,
                start_angle,
                sweep_angle: -PI,
            }
        } else {
            CurveEdge2::Line {
                from: side(point, sign),
                to: side(point, -sign),
            }
        }
    };
    CurveRegion2 {
        outer: vec![
            CurveEdge2::Line {
                from: side(a, 1.0),
                to: side(b, 1.0),
            },
            cap(b, facing, 1.0, caps[1]),
            CurveEdge2::Line {
                from: side(b, -1.0),
                to: side(a, -1.0),
            },
            cap(a, facing + PI, -1.0, caps[0]),
        ],
        holes: Vec::new(),
    }
}

fn disk(centre: Pt2, radius: f64) -> CurveRegion2 {
    let half = |start_angle: f64| CurveEdge2::Arc {
        center: [centre.x, centre.z],
        radius,
        start_angle,
        sweep_angle: -PI,
    };
    CurveRegion2 {
        outer: vec![half(0.0), half(-PI)],
        holes: Vec::new(),
    }
}

fn middle(a: Pt2, b: Pt2) -> Pt2 {
    Pt2::new((a.x + b.x) * 0.5, (a.z + b.z) * 0.5)
}

fn thin(ring: &[CurveEdge2], eps: f64) -> bool {
    let area = ring.iter().map(CurveEdge2::twice_area).sum::<f64>() * 0.5;
    let perimeter: f64 = ring.iter().map(CurveEdge2::length).sum();
    area.abs() < eps * perimeter * 0.5
}

fn length(a: Pt2, b: Pt2) -> f64 {
    (b.x - a.x).hypot(b.z - a.z)
}
