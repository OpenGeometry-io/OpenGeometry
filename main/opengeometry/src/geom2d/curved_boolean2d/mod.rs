mod edge;

pub(crate) use edge::{intersections, reversed_ring, CurveEdge2, CurveRegion2};

use super::boolean2d::PlanarBooleanOp;
use super::poly2d::Pt2;
use crate::brep::GeometryError;
use edge::{add, canonicalize_ring, compare_points, cross, distance, dot, p, scale};
use std::collections::{HashMap, HashSet};
use std::f64::consts::TAU;

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
    let filled = |point: Pt2| {
        let left = inside(point, a, eps);
        let right = inside(point, b, eps);
        match operation {
            PlanarBooleanOp::Union => left || right,
            PlanarBooleanOp::Intersection => left && right,
            PlanarBooleanOp::Subtraction => left && !right,
        }
    };
    let split = subdivided_edges(&edges, eps);
    let boundary = boundary_edges(split, eps, filled);
    let loops = boundary_loops(&boundary, eps)?;
    let mut result = nested_regions(&loops, eps);
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

fn inside(point: Pt2, regions: &[CurveRegion2], eps: f64) -> bool {
    regions.iter().any(|region| {
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
            CurveEdge2::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                let sine = (point.z - center[1]) / radius;
                if sine.abs() >= 1.0 {
                    continue;
                }
                let first = sine.asin();
                for angle in [first, std::f64::consts::PI - first] {
                    let hit = Pt2::new(center[0] + radius * angle.cos(), point.z);
                    if hit.x <= point.x + eps * 0.01 {
                        continue;
                    }
                    let travel = if *sweep_angle > 0.0 {
                        (angle - start_angle).rem_euclid(TAU)
                    } else {
                        (start_angle - angle).rem_euclid(TAU)
                    };
                    let dz = radius * sweep_angle * angle.cos();
                    if travel > sweep_angle.abs() + eps / radius
                        || (travel <= eps / radius && dz < 0.0)
                        || (travel >= sweep_angle.abs() - eps / radius && dz > 0.0)
                    {
                        continue;
                    }
                    if dz.abs() > eps * radius {
                        total += if dz > 0.0 { 1 } else { -1 };
                    }
                }
            }
        }
    }
    total
}

fn subdivided_edges(edges: &[CurveEdge2], eps: f64) -> Vec<CurveEdge2> {
    let mut split = Vec::new();
    for (index, edge) in edges.iter().enumerate() {
        let mut ts = vec![0.0, 1.0];
        for (other_index, other) in edges.iter().enumerate() {
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

fn boundary_edges(
    split: Vec<CurveEdge2>,
    eps: f64,
    filled: impl Fn(Pt2) -> bool,
) -> Vec<CurveEdge2> {
    let mut boundary = Vec::new();
    let mut seen = HashSet::new();
    for edge in split {
        let left = filled(probe(&edge, 1.0, eps));
        let right = filled(probe(&edge, -1.0, eps));
        if left == right {
            continue;
        }
        let oriented = if left { edge } else { edge.reversed() };
        let key = (
            quant(oriented.point(0.0), eps),
            quant(oriented.point(0.5), eps),
            quant(oriented.point(1.0), eps),
        );
        if seen.insert(key) {
            boundary.push(oriented);
        }
    }
    boundary
}

fn probe(edge: &CurveEdge2, side: f64, eps: f64) -> Pt2 {
    let mid = edge.point(0.5);
    let tangent = edge.tangent(0.5);
    let length = tangent.x.hypot(tangent.z);
    let distance = (edge.length() * 0.1).min(1.0e-4).max(eps * 2.0);
    add(
        mid,
        Pt2::new(
            -side * tangent.z * distance / length,
            side * tangent.x * distance / length,
        ),
    )
}

fn quant(point: Pt2, eps: f64) -> (i64, i64) {
    let step = eps * 2.0;
    (
        (point.x / step).round() as i64,
        (point.z / step).round() as i64,
    )
}

fn boundary_loops(
    boundary: &[CurveEdge2],
    eps: f64,
) -> Result<Vec<Vec<CurveEdge2>>, GeometryError> {
    let mut outgoing: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (index, edge) in boundary.iter().enumerate() {
        outgoing
            .entry(quant(edge.point(0.0), eps))
            .or_default()
            .push(index);
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
            let at = quant(edge.point(1.0), eps);
            if at == quant(boundary[start].point(0.0), eps) {
                break;
            }
            let reverse = scale(edge.tangent(1.0), -1.0);
            let mut best = None;
            let mut best_turn = f64::INFINITY;
            for &candidate in outgoing.get(&at).map(Vec::as_slice).unwrap_or(&[]) {
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

fn nested_regions(loops: &[Vec<CurveEdge2>], eps: f64) -> Vec<CurveRegion2> {
    let mut result: Vec<CurveRegion2> = loops
        .iter()
        .filter(|ring| ring_area(ring) > 0.0)
        .map(|ring| CurveRegion2 {
            outer: reversed_ring(ring),
            holes: Vec::new(),
        })
        .collect();
    for hole in loops.iter().filter(|ring| ring_area(ring) < 0.0) {
        let sample = probe(&hole[0], -1.0, eps);
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
        if let Some(index) = index {
            result[index].holes.push(reversed_ring(hole));
        }
    }
    result
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
