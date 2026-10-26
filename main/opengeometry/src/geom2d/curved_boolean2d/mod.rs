mod edge;

pub(crate) use edge::{intersections, reversed_ring, CurveEdge2, CurveRegion2};

use super::boolean2d::{PlanarBooleanOp, VertexWeld};
use super::poly2d::Pt2;
use crate::brep::GeometryError;
use edge::{add, canonicalize_ring, compare_points, cross, distance, dot, p, scale, Boxes};
use std::collections::HashMap;
use std::f64::consts::TAU;

struct CurveBoundary {
    edges: Vec<CurveEdge2>,
    ends: Vec<(usize, usize)>,
    vertex_count: usize,
}

pub(crate) fn boolean_curved_regions(
    a: &[CurveRegion2],
    b: &[CurveRegion2],
    operation: PlanarBooleanOp,
    eps: f64,
) -> Result<Vec<CurveRegion2>, GeometryError> {
    if !eps.is_finite() || eps <= 0.0 {
        return unresolved("Invalid Boolean tolerance");
    }
    check_regions(a, eps)?;
    check_regions(b, eps)?;
    let edges: Vec<CurveEdge2> = a
        .iter()
        .chain(b)
        .flat_map(|region| {
            std::iter::once(&region.outer)
                .chain(region.holes.iter())
                .flat_map(|ring| ring.iter().cloned())
        })
        .collect();
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    let a_bounds = region_bounds(a, eps);
    let b_bounds = region_bounds(b, eps);
    let filled = |point: Pt2| {
        let left = inside(point, a, &a_bounds, eps);
        let right = inside(point, b, &b_bounds, eps);
        match operation {
            PlanarBooleanOp::Union => left || right,
            PlanarBooleanOp::Intersection => left && right,
            PlanarBooleanOp::Subtraction => left && !right,
        }
    };
    let split = subdivided_edges(&edges, eps);
    let boundary = boundary_edges(split, eps, filled);
    let loops = boundary_loops(&boundary, eps)?;
    let mut result = nested_regions(&loops, eps)?;
    canonicalize_regions(&mut result);
    Ok(result)
}

fn unresolved<T>(reason: &str) -> Result<T, GeometryError> {
    Err(GeometryError::UnresolvedIntersection(reason.into()))
}

fn check_regions(regions: &[CurveRegion2], eps: f64) -> Result<(), GeometryError> {
    for region in regions {
        validate_ring(&region.outer, false, eps)?;
        for hole in &region.holes {
            validate_ring(hole, true, eps)?;
        }
    }
    Ok(())
}

fn validate_ring(
    ring: &[CurveEdge2],
    expected_positive: bool,
    eps: f64,
) -> Result<(), GeometryError> {
    if ring.len() < 2 {
        return unresolved("Curved Boolean ring needs at least two edges");
    }
    for (index, edge) in ring.iter().enumerate() {
        if !edge.length().is_finite()
            || edge.length() <= 4.0 * eps
            || ![
                edge.point(0.0).x,
                edge.point(0.0).z,
                edge.point(1.0).x,
                edge.point(1.0).z,
            ]
            .iter()
            .all(|v| v.is_finite())
        {
            return unresolved("Curved Boolean edge is unresolved");
        }
        if let CurveEdge2::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } = edge
        {
            if !center.iter().all(|v| v.is_finite())
                || !radius.is_finite()
                || !start_angle.is_finite()
                || !sweep_angle.is_finite()
                || *radius <= 4.0 * eps
                || sweep_angle.abs() >= TAU
            {
                return unresolved("Curved Boolean arc is invalid");
            }
        }
        let next = &ring[(index + 1) % ring.len()];
        if distance(edge.point(1.0), next.point(0.0)) > eps {
            return unresolved("Curved Boolean ring is open");
        }
    }
    let area = ring_area(ring);
    if area.abs() <= eps * eps || (area > 0.0) != expected_positive {
        return unresolved("Curved Boolean ring has degenerate or incorrect winding");
    }
    Ok(())
}

fn ring_area(ring: &[CurveEdge2]) -> f64 {
    ring.iter().map(CurveEdge2::twice_area).sum::<f64>() * 0.5
}

fn region_bounds(regions: &[CurveRegion2], eps: f64) -> Boxes {
    Boxes::new(
        regions
            .iter()
            .map(|region| {
                region.outer.iter().fold(
                    [
                        f64::INFINITY,
                        f64::INFINITY,
                        f64::NEG_INFINITY,
                        f64::NEG_INFINITY,
                    ],
                    |total, edge| {
                        let bounds = edge.bounds(eps);
                        [
                            total[0].min(bounds[0]),
                            total[1].min(bounds[1]),
                            total[2].max(bounds[2]),
                            total[3].max(bounds[3]),
                        ]
                    },
                )
            })
            .collect(),
    )
}

fn around(point: Pt2, reach: f64) -> [f64; 4] {
    [
        point.x - reach,
        point.z - reach,
        point.x + reach,
        point.z + reach,
    ]
}

fn inside(point: Pt2, regions: &[CurveRegion2], bounds: &Boxes, eps: f64) -> bool {
    bounds
        .touching([point.x, point.z, f64::INFINITY, point.z])
        .any(|index| {
            let region = &regions[index];
            winding(point, &region.outer, eps) != 0
                && !region
                    .holes
                    .iter()
                    .any(|hole| winding(point, hole, eps) != 0)
        })
}

pub(crate) fn winding(point: Pt2, ring: &[CurveEdge2], eps: f64) -> i32 {
    let mut total = 0;
    for edge in ring {
        match edge {
            CurveEdge2::Line { from, to } => {
                let a = p(*from);
                let b = p(*to);
                let az = if (a.z - point.z).abs() <= eps * 0.25 {
                    point.z
                } else {
                    a.z
                };
                let bz = if (b.z - point.z).abs() <= eps * 0.25 {
                    point.z
                } else {
                    b.z
                };
                if (az <= point.z && bz > point.z) || (bz <= point.z && az > point.z) {
                    let x = a.x + (point.z - az) * (b.x - a.x) / (bz - az);
                    if x > point.x + eps * 0.01 {
                        total += if bz > az { 1 } else { -1 };
                    }
                }
            }
            CurveEdge2::Arc { .. } => total += arc_winding(point, edge, eps),
        }
    }
    total
}

fn arc_winding(point: Pt2, arc: &CurveEdge2, eps: f64) -> i32 {
    let CurveEdge2::Arc {
        center,
        radius,
        start_angle,
        sweep_angle,
    } = arc
    else {
        return 0;
    };
    let start = arc.point(0.0).z;
    let end = arc.point(1.0).z;
    let at_start = (start - point.z).abs() <= eps * 0.25;
    let at_end = (end - point.z).abs() <= eps * 0.25;
    let level = if at_start {
        start
    } else if at_end {
        end
    } else {
        point.z
    };
    let sine = (level - center[1]) / radius;
    if sine.abs() >= 1.0 && !at_start && !at_end {
        return 0;
    }
    let sine = sine.clamp(-1.0, 1.0);
    let delta = eps / radius;
    let sweep = sweep_angle.abs();
    let first = sine.asin();
    let mut total = 0;
    let angles = if sine.abs() >= 1.0 {
        vec![first]
    } else {
        vec![first, std::f64::consts::PI - first]
    };
    for angle in angles {
        if center[0] + radius * angle.cos() <= point.x + eps * 0.01 {
            continue;
        }
        let travel = if *sweep_angle > 0.0 {
            (angle - start_angle).rem_euclid(TAU)
        } else {
            (start_angle - angle).rem_euclid(TAU)
        };
        let dz = radius * sweep_angle * angle.cos();
        let flat = dz.abs() <= eps * radius;
        let below = i32::from(center[1] > level);
        if flat && at_start && (travel <= delta || travel >= TAU - delta) {
            total += below;
            continue;
        }
        if flat && at_end && (travel - sweep).abs() <= delta {
            total -= below;
            continue;
        }
        let crossed = if at_start && (travel <= delta || travel >= TAU - delta) {
            dz > 0.0
        } else if at_end && (travel - sweep).abs() <= delta {
            dz < 0.0
        } else {
            travel <= sweep
        };
        if crossed && dz.abs() > eps * radius {
            total += if dz > 0.0 { 1 } else { -1 };
        }
    }
    total
}

fn subdivided_edges(edges: &[CurveEdge2], eps: f64) -> Vec<CurveEdge2> {
    let mut split = Vec::new();
    let bounds = Boxes::new(edges.iter().map(|edge| edge.bounds(2.0 * eps)).collect());
    for (index, edge) in edges.iter().enumerate() {
        let mut ts = vec![0.0, 1.0];
        for other_index in bounds.touching(bounds.of(index)) {
            let other = &edges[other_index];
            if index == other_index {
                continue;
            }
            for point in intersections(edge, other, eps) {
                if let Some(t) = edge.parameter(point, eps) {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(|x, y| x.total_cmp(y));
        let mut clean: Vec<f64> = Vec::new();
        for t in ts {
            if clean
                .last()
                .is_none_or(|last| (t - last) * edge.length() > eps)
            {
                clean.push(t);
            }
        }
        if clean
            .last()
            .is_some_and(|last| 1.0 - last > eps / edge.length())
        {
            clean.push(1.0);
        }
        for window in clean.windows(2) {
            let piece = edge.slice(window[0], window[1]);
            if piece.length() > eps {
                split.push(piece);
            }
        }
    }
    split
}

fn boundary_edges(split: Vec<CurveEdge2>, eps: f64, filled: impl Fn(Pt2) -> bool) -> CurveBoundary {
    let mut weld = VertexWeld::new(eps);
    let mut edges = Vec::new();
    let mut ends = Vec::new();
    let mut seen: HashMap<(usize, usize), Vec<Pt2>> = HashMap::new();
    let bounds = Boxes::new(split.iter().map(|edge| edge.bounds(eps)).collect());
    for (index, edge) in split.iter().enumerate() {
        let (at, reach) = probe_site(edge, index, &split, &bounds, eps);
        let left = filled(probe(edge, at, 1.0, reach));
        let right = filled(probe(edge, at, -1.0, reach));
        if left == right {
            continue;
        }
        let oriented = if left { edge.clone() } else { edge.reversed() };
        let key = (
            weld.weld(oriented.point(0.0)),
            weld.weld(oriented.point(1.0)),
        );
        let middle = oriented.point(0.5);
        let middles = seen.entry(key).or_default();
        if key.0 != key.1 && middles.iter().all(|other| distance(*other, middle) > eps) {
            middles.push(middle);
            ends.push(key);
            edges.push(oriented);
        }
    }
    CurveBoundary {
        edges,
        ends,
        vertex_count: weld.vertex_count(),
    }
}

fn probe_site(
    edge: &CurveEdge2,
    index: usize,
    others: &[CurveEdge2],
    bounds: &Boxes,
    eps: f64,
) -> (f64, f64) {
    for at in [0.5, 0.25, 0.75, 0.125, 0.875] {
        let (reach, crowded) =
            probe_reach(edge.point(at), edge, Some(index), (others, bounds), eps);
        if !crowded {
            return (at, reach);
        }
    }
    (
        0.5,
        probe_reach(edge.point(0.5), edge, None, (others, bounds), eps).0,
    )
}

fn probe_reach(
    mid: Pt2,
    edge: &CurveEdge2,
    own: Option<usize>,
    (others, bounds): (&[CurveEdge2], &Boxes),
    eps: f64,
) -> (f64, bool) {
    let mut clearance = 2.0 * (edge.length() * 0.1).min(1.0e-4).max(eps * 2.0);
    let reach = clearance;
    let mut crowded = false;
    for index in bounds.touching(around(mid, reach)) {
        let other = &others[index];
        let distance = other.distance_to(mid);
        if distance > eps && distance < clearance {
            clearance = distance;
        }
        crowded |= distance <= eps && own.is_some_and(|own| own != index);
    }
    (clearance * 0.5, crowded)
}

fn probe(edge: &CurveEdge2, at: f64, side: f64, distance: f64) -> Pt2 {
    let mid = edge.point(at);
    let tangent = edge.tangent(at);
    let length = tangent.x.hypot(tangent.z);
    add(
        mid,
        Pt2::new(
            -side * tangent.z * distance / length,
            side * tangent.x * distance / length,
        ),
    )
}

fn boundary_loops(
    curve_boundary: &CurveBoundary,
    eps: f64,
) -> Result<Vec<Vec<CurveEdge2>>, GeometryError> {
    let boundary = &curve_boundary.edges;
    let ends = &curve_boundary.ends;
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); curve_boundary.vertex_count];
    for (index, &(start, _)) in ends.iter().enumerate() {
        outgoing[start].push(index);
    }
    let mut used = vec![false; boundary.len()];
    let mut loops = Vec::new();
    for start in 0..boundary.len() {
        if used[start] {
            continue;
        }
        let mut current = start;
        let mut ring = Vec::new();
        for _ in 0..=boundary.len() {
            if used[current] {
                break;
            }
            used[current] = true;
            let edge = &boundary[current];
            ring.push(edge.clone());
            let at = ends[current].1;
            if at == ends[start].0 {
                break;
            }
            let reverse = scale(edge.tangent(1.0), -1.0);
            let mut best = None;
            let mut best_turn = f64::INFINITY;
            for &candidate in &outgoing[at] {
                if used[candidate] {
                    continue;
                }
                let direction = boundary[candidate].tangent(0.0);
                let mut turn = -cross(reverse, direction).atan2(dot(reverse, direction));
                if turn <= 1.0e-12 {
                    turn += TAU;
                }
                if turn < best_turn {
                    best_turn = turn;
                    best = Some(candidate);
                }
            }
            match best {
                Some(next) => current = next,
                None => break,
            }
        }
        if ring.len() < 2
            || ring
                .last()
                .is_none_or(|last| distance(last.point(1.0), ring[0].point(0.0)) > eps)
            || ring_area(&ring).abs() <= eps * eps
        {
            return unresolved("Curved Boolean produced an open or degenerate boundary");
        }
        loops.push(ring);
    }
    Ok(loops)
}

pub(super) fn nested_regions(
    loops: &[Vec<CurveEdge2>],
    eps: f64,
) -> Result<Vec<CurveRegion2>, GeometryError> {
    let mut result: Vec<CurveRegion2> = loops
        .iter()
        .filter(|ring| ring_area(ring) > 0.0)
        .map(|ring| CurveRegion2 {
            outer: reversed_ring(ring),
            holes: Vec::new(),
        })
        .collect();
    let edges = loops.concat();
    let bounds = Boxes::new(edges.iter().map(|edge| edge.bounds(eps)).collect());
    let mut first = 0;
    for ring in loops {
        first += ring.len();
        if ring_area(ring) >= 0.0 {
            continue;
        }
        let hole = ring;
        let (at, reach) = probe_site(&hole[0], first - hole.len(), &edges, &bounds, eps);
        let sample = probe(&hole[0], at, -1.0, reach);
        let index = result
            .iter()
            .enumerate()
            .filter(|(_, region)| winding(sample, &region.outer, eps) != 0)
            .min_by(|(_, first), (_, second)| {
                ring_area(&first.outer)
                    .abs()
                    .total_cmp(&ring_area(&second.outer).abs())
            })
            .map(|(index, _)| index);
        let Some(index) = index else {
            return Err(GeometryError::InvalidTopology(
                "Curved Boolean produced a hole outside every region".into(),
            ));
        };
        result[index].holes.push(reversed_ring(hole));
    }
    Ok(result)
}

fn canonicalize_regions(result: &mut [CurveRegion2]) {
    for region in result.iter_mut() {
        canonicalize_ring(&mut region.outer);
        for hole in &mut region.holes {
            canonicalize_ring(hole);
        }
        region
            .holes
            .sort_by(|a, b| compare_points(a[0].point(0.0), b[0].point(0.0)));
    }
    result.sort_by(|a, b| compare_points(a.outer[0].point(0.0), b.outer[0].point(0.0)));
}
