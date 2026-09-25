//! Analytic 2D region Boolean for closed line/arc rings in the XZ plane.
//! Curves are split only at exact line/circle and circle/circle intersections;
//! no chords are introduced into the returned boundary.

use super::boolean2d::PlanarBooleanOp;
use super::poly2d::Pt2;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::f64::consts::TAU;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CurveEdge2 {
    Line {
        from: [f64; 2],
        to: [f64; 2],
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CurveRegion2 {
    pub outer: Vec<CurveEdge2>,
    #[serde(default)]
    pub holes: Vec<Vec<CurveEdge2>>,
}

fn p(a: [f64; 2]) -> Pt2 {
    Pt2::new(a[0], a[1])
}
fn arr(a: Pt2) -> [f64; 2] {
    [a.x, a.z]
}
fn add(a: Pt2, b: Pt2) -> Pt2 {
    Pt2::new(a.x + b.x, a.z + b.z)
}
fn sub(a: Pt2, b: Pt2) -> Pt2 {
    Pt2::new(a.x - b.x, a.z - b.z)
}
fn scale(a: Pt2, s: f64) -> Pt2 {
    Pt2::new(a.x * s, a.z * s)
}
fn dot(a: Pt2, b: Pt2) -> f64 {
    a.x * b.x + a.z * b.z
}
fn cross(a: Pt2, b: Pt2) -> f64 {
    a.x * b.z - a.z * b.x
}
fn distance(a: Pt2, b: Pt2) -> f64 {
    sub(a, b).x.hypot(sub(a, b).z)
}

fn compare_points(a: Pt2, b: Pt2) -> std::cmp::Ordering {
    a.x.total_cmp(&b.x).then_with(|| a.z.total_cmp(&b.z))
}

fn canonicalize_ring(ring: &mut Vec<CurveEdge2>) {
    if let Some((index, _)) = ring.iter().enumerate().min_by(|(_, first), (_, second)| {
        compare_points(first.point(0.0), second.point(0.0))
            .then_with(|| compare_points(first.point(0.5), second.point(0.5)))
            .then_with(|| compare_points(first.point(1.0), second.point(1.0)))
    }) {
        ring.rotate_left(index);
    }
}

impl CurveEdge2 {
    pub fn point(&self, t: f64) -> Pt2 {
        match self {
            Self::Line { from, to } => add(p(*from), scale(sub(p(*to), p(*from)), t)),
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                let angle = start_angle + sweep_angle * t;
                Pt2::new(
                    center[0] + radius * angle.cos(),
                    center[1] + radius * angle.sin(),
                )
            }
        }
    }

    pub(crate) fn tangent(&self, t: f64) -> Pt2 {
        match self {
            Self::Line { from, to } => sub(p(*to), p(*from)),
            Self::Arc {
                radius,
                start_angle,
                sweep_angle,
                ..
            } => {
                let angle = start_angle + sweep_angle * t;
                Pt2::new(
                    -radius * sweep_angle * angle.sin(),
                    radius * sweep_angle * angle.cos(),
                )
            }
        }
    }

    pub(crate) fn length(&self) -> f64 {
        match self {
            Self::Line { .. } => distance(self.point(0.0), self.point(1.0)),
            Self::Arc {
                radius,
                sweep_angle,
                ..
            } => radius * sweep_angle.abs(),
        }
    }

    pub(crate) fn reverse(&self) -> Self {
        match self {
            Self::Line { from, to } => Self::Line {
                from: *to,
                to: *from,
            },
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => Self::Arc {
                center: *center,
                radius: *radius,
                start_angle: start_angle + sweep_angle,
                sweep_angle: -*sweep_angle,
            },
        }
    }

    pub(crate) fn slice(&self, a: f64, b: f64) -> Self {
        match self {
            Self::Line { .. } => Self::Line {
                from: arr(self.point(a)),
                to: arr(self.point(b)),
            },
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => Self::Arc {
                center: *center,
                radius: *radius,
                start_angle: start_angle + sweep_angle * a,
                sweep_angle: sweep_angle * (b - a),
            },
        }
    }

    pub(crate) fn parameter(&self, point: Pt2, eps: f64) -> Option<f64> {
        match self {
            Self::Line { from, to } => {
                let d = sub(p(*to), p(*from));
                let len2 = dot(d, d);
                if len2 <= eps * eps {
                    return None;
                }
                let t = dot(sub(point, p(*from)), d) / len2;
                (t >= -eps / len2.sqrt()
                    && t <= 1.0 + eps / len2.sqrt()
                    && distance(self.point(t), point) <= eps)
                    .then_some(t.clamp(0.0, 1.0))
            }
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                let radial = sub(point, p(*center));
                if (radial.x.hypot(radial.z) - radius).abs() > eps {
                    return None;
                }
                let angle = radial.z.atan2(radial.x);
                let travel = if *sweep_angle > 0.0 {
                    (angle - start_angle).rem_euclid(TAU)
                } else {
                    (start_angle - angle).rem_euclid(TAU)
                };
                let delta = eps / radius;
                if travel <= sweep_angle.abs() + delta {
                    Some((travel / sweep_angle.abs()).clamp(0.0, 1.0))
                } else if TAU - travel <= delta {
                    Some(0.0)
                } else {
                    None
                }
            }
        }
    }

    pub(crate) fn twice_area(&self) -> f64 {
        let a = self.point(0.0);
        let b = self.point(1.0);
        match self {
            Self::Line { .. } => cross(a, b),
            Self::Arc {
                center,
                radius,
                sweep_angle,
                ..
            } => radius * radius * sweep_angle + center[0] * (b.z - a.z) - center[1] * (b.x - a.x),
        }
    }
}

pub(crate) fn intersections(a: &CurveEdge2, b: &CurveEdge2, eps: f64) -> Vec<Pt2> {
    let mut candidates = Vec::new();
    let mut offer = |hit: Pt2| {
        if a.parameter(hit, eps).is_some()
            && b.parameter(hit, eps).is_some()
            && !candidates.iter().any(|prior| distance(*prior, hit) <= eps)
        {
            candidates.push(hit);
        }
    };
    match (a, b) {
        (CurveEdge2::Line { from, to }, CurveEdge2::Line { from: bf, to: bt }) => {
            let d = sub(p(*to), p(*from));
            let e = sub(p(*bt), p(*bf));
            let denom = cross(d, e);
            if denom.abs() > eps * d.x.hypot(d.z) * e.x.hypot(e.z) {
                offer(add(
                    p(*from),
                    scale(d, cross(sub(p(*bf), p(*from)), e) / denom),
                ));
            } else {
                for endpoint in [*from, *to, *bf, *bt] {
                    offer(p(endpoint));
                }
            }
        }
        (CurveEdge2::Line { from, to }, CurveEdge2::Arc { center, radius, .. })
        | (CurveEdge2::Arc { center, radius, .. }, CurveEdge2::Line { from, to }) => {
            let d = sub(p(*to), p(*from));
            let f = sub(p(*from), p(*center));
            let aa = dot(d, d);
            if aa <= eps * eps {
                return candidates;
            }
            let bb = 2.0 * dot(f, d);
            let cc = dot(f, f) - radius * radius;
            let disc = bb * bb - 4.0 * aa * cc;
            if disc >= -eps * eps * aa {
                let root = disc.max(0.0).sqrt();
                for t in [(-bb - root) / (2.0 * aa), (-bb + root) / (2.0 * aa)] {
                    offer(add(p(*from), scale(d, t)));
                }
            }
        }
        (
            CurveEdge2::Arc {
                center: ca,
                radius: ra,
                ..
            },
            CurveEdge2::Arc {
                center: cb,
                radius: rb,
                ..
            },
        ) => {
            let centre_delta = sub(p(*cb), p(*ca));
            let d = centre_delta.x.hypot(centre_delta.z);
            if d <= eps && (ra - rb).abs() <= eps {
                for endpoint in [a.point(0.0), a.point(1.0), b.point(0.0), b.point(1.0)] {
                    offer(endpoint);
                }
            } else if d > eps && d <= ra + rb + eps && d + ra.min(*rb) + eps >= ra.max(*rb) {
                let along = (ra * ra - rb * rb + d * d) / (2.0 * d);
                let height2 = ra * ra - along * along;
                if height2 >= -eps * eps {
                    let u = scale(centre_delta, 1.0 / d);
                    let base = add(p(*ca), scale(u, along));
                    let normal = Pt2::new(-u.z, u.x);
                    let h = height2.max(0.0).sqrt();
                    offer(add(base, scale(normal, h)));
                    offer(add(base, scale(normal, -h)));
                }
            }
        }
    }
    candidates
}

fn ring_area(ring: &[CurveEdge2]) -> f64 {
    ring.iter().map(CurveEdge2::twice_area).sum::<f64>() * 0.5
}

fn validate_ring(ring: &[CurveEdge2], expected_positive: bool, eps: f64) -> Result<(), String> {
    if ring.len() < 2 {
        return Err("Curved Boolean ring needs at least two edges".into());
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
            return Err("Curved Boolean edge is unresolved".into());
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
                return Err("Curved Boolean arc is invalid".into());
            }
        }
        let next = &ring[(index + 1) % ring.len()];
        if distance(edge.point(1.0), next.point(0.0)) > eps {
            return Err("Curved Boolean ring is open".into());
        }
    }
    let area = ring_area(ring);
    if area.abs() <= eps * eps || (area > 0.0) != expected_positive {
        return Err("Curved Boolean ring has degenerate or incorrect winding".into());
    }
    Ok(())
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

fn quant(point: Pt2, eps: f64) -> (i64, i64) {
    let step = eps * 2.0;
    (
        (point.x / step).round() as i64,
        (point.z / step).round() as i64,
    )
}

fn reverse_ring(ring: &[CurveEdge2]) -> Vec<CurveEdge2> {
    ring.iter().rev().map(CurveEdge2::reverse).collect()
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

/// Boolean regions with analytic line and circular-arc boundaries. Input and
/// output use CW outer rings and CCW holes, matching `booleanRegions2D`.
pub fn boolean_curved_regions(
    a: &[CurveRegion2],
    b: &[CurveRegion2],
    operation: PlanarBooleanOp,
    eps: f64,
) -> Result<Vec<CurveRegion2>, String> {
    if !eps.is_finite() || eps <= 0.0 {
        return Err("Invalid Boolean tolerance".into());
    }
    let validate = |regions: &[CurveRegion2]| -> Result<(), String> {
        for region in regions {
            validate_ring(&region.outer, false, eps)?;
            for hole in &region.holes {
                validate_ring(hole, true, eps)?;
            }
        }
        Ok(())
    };
    validate(a)?;
    validate(b)?;
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
    let inside = |point: Pt2, regions: &[CurveRegion2]| -> bool {
        regions.iter().any(|region| {
            winding(point, &region.outer, eps) != 0
                && !region
                    .holes
                    .iter()
                    .any(|hole| winding(point, hole, eps) != 0)
        })
    };
    let filled = |point: Pt2| {
        let left = inside(point, a);
        let right = inside(point, b);
        match operation {
            PlanarBooleanOp::Union => left || right,
            PlanarBooleanOp::Intersection => left && right,
            PlanarBooleanOp::Subtraction => left && !right,
        }
    };
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
    let mut boundary = Vec::new();
    let mut seen = HashSet::new();
    for edge in split {
        let left = filled(probe(&edge, 1.0, eps));
        let right = filled(probe(&edge, -1.0, eps));
        if left == right {
            continue;
        }
        let oriented = if left { edge } else { edge.reverse() };
        let key = (
            quant(oriented.point(0.0), eps),
            quant(oriented.point(0.5), eps),
            quant(oriented.point(1.0), eps),
        );
        if seen.insert(key) {
            boundary.push(oriented);
        }
    }
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
            || distance(ring.last().unwrap().point(1.0), ring[0].point(0.0)) > eps
            || ring_area(&ring).abs() <= eps * eps
        {
            return Err("Curved Boolean produced an open or degenerate boundary".into());
        }
        loops.push(ring);
    }
    let mut result: Vec<CurveRegion2> = loops
        .iter()
        .filter(|ring| ring_area(ring) > 0.0)
        .map(|ring| CurveRegion2 {
            outer: reverse_ring(ring),
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
            result[index].holes.push(reverse_ring(hole));
        }
    }
    for region in &mut result {
        canonicalize_ring(&mut region.outer);
        for hole in &mut region.holes {
            canonicalize_ring(hole);
        }
        region
            .holes
            .sort_by(|a, b| compare_points(a[0].point(0.0), b[0].point(0.0)));
    }
    result.sort_by(|a, b| compare_points(a.outer[0].point(0.0), b.outer[0].point(0.0)));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sector(start: f64, end: f64) -> CurveRegion2 {
        let at = |radius: f64, angle: f64| [radius * angle.cos(), radius * angle.sin()];
        CurveRegion2 {
            outer: vec![
                CurveEdge2::Arc {
                    center: [0.0, 0.0],
                    radius: 2.0,
                    start_angle: end,
                    sweep_angle: start - end,
                },
                CurveEdge2::Line {
                    from: at(2.0, start),
                    to: at(1.0, start),
                },
                CurveEdge2::Arc {
                    center: [0.0, 0.0],
                    radius: 1.0,
                    start_angle: start,
                    sweep_angle: end - start,
                },
                CurveEdge2::Line {
                    from: at(1.0, end),
                    to: at(2.0, end),
                },
            ],
            holes: Vec::new(),
        }
    }

    fn rectangle(x0: f64, z0: f64, x1: f64, z1: f64) -> CurveRegion2 {
        let points = [[x0, z0], [x0, z1], [x1, z1], [x1, z0]];
        CurveRegion2 {
            outer: (0..4)
                .map(|index| CurveEdge2::Line {
                    from: points[index],
                    to: points[(index + 1) % 4],
                })
                .collect(),
            holes: Vec::new(),
        }
    }

    fn area(regions: &[CurveRegion2]) -> f64 {
        regions
            .iter()
            .map(|region| {
                -ring_area(&region.outer)
                    - region.holes.iter().map(|hole| ring_area(hole)).sum::<f64>()
            })
            .sum()
    }

    #[test]
    fn coincident_arc_boundaries_preserve_exact_overlap() {
        let a = sector(0.0, std::f64::consts::FRAC_PI_2);
        let b = sector(
            std::f64::consts::FRAC_PI_4,
            3.0 * std::f64::consts::FRAC_PI_4,
        );
        let expected = 1.5 * std::f64::consts::FRAC_PI_4;
        for (operation, target) in [
            (PlanarBooleanOp::Intersection, expected),
            (
                PlanarBooleanOp::Union,
                1.5 * 3.0 * std::f64::consts::FRAC_PI_4,
            ),
            (PlanarBooleanOp::Subtraction, expected),
        ] {
            let out = boolean_curved_regions(&[a.clone()], &[b.clone()], operation, 1e-7).unwrap();
            assert_eq!(out.len(), 1);
            assert!(
                (area(&out) - target).abs() < 1e-6,
                "{operation:?}: expected {target}, got {}",
                area(&out)
            );
            assert!(out[0]
                .outer
                .iter()
                .any(|edge| matches!(edge, CurveEdge2::Arc { .. })));
        }
    }

    #[test]
    fn line_arc_cut_splits_sector_without_chording() {
        let base = sector(0.0, std::f64::consts::FRAC_PI_2);
        let cutter = rectangle(1.2, -0.2, 1.8, 2.2);
        let out = boolean_curved_regions(
            &[base.clone()],
            &[cutter],
            PlanarBooleanOp::Subtraction,
            1e-7,
        )
        .unwrap();
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|region| ring_area(&region.outer) < 0.0));
        assert!(
            out.iter()
                .flat_map(|region| &region.outer)
                .filter(|edge| matches!(edge, CurveEdge2::Arc { .. }))
                .count()
                >= 2
        );
        assert!(area(&out) > 0.0 && area(&out) < area(&[base]));
    }

    #[test]
    fn identical_arc_region_union_and_subtraction_are_canonical() {
        let source = sector(-0.4, 1.7);
        let union = boolean_curved_regions(
            &[source.clone()],
            &[source.clone()],
            PlanarBooleanOp::Union,
            1e-7,
        )
        .unwrap();
        assert_eq!(union.len(), 1);
        assert!((area(&union) - area(&[source.clone()])).abs() < 1e-6);
        let difference = boolean_curved_regions(
            &[source.clone()],
            &[source],
            PlanarBooleanOp::Subtraction,
            1e-7,
        )
        .unwrap();
        assert!(difference.is_empty());
    }

    #[test]
    fn random_concentric_sector_booleans_match_analytic_areas() {
        let mut seed = 0x8a53_71de_u64;
        let mut random = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed >> 32) as f64) / (u32::MAX as f64)
        };
        for case in 0..120 {
            let a0 = -0.7 + 1.4 * random();
            let a1 = a0 + 0.2 + 1.0 * random();
            let b0 = -0.7 + 1.4 * random();
            let b1 = b0 + 0.2 + 1.0 * random();
            let a = sector(a0, a1);
            let b = sector(b0, b1);
            let overlap = (a1.min(b1) - a0.max(b0)).max(0.0);
            for (operation, expected) in [
                (PlanarBooleanOp::Union, 1.5 * (a1 - a0 + b1 - b0 - overlap)),
                (PlanarBooleanOp::Intersection, 1.5 * overlap),
                (PlanarBooleanOp::Subtraction, 1.5 * (a1 - a0 - overlap)),
            ] {
                let out =
                    boolean_curved_regions(&[a.clone()], &[b.clone()], operation, 1e-7).unwrap();
                assert!(
                    (area(&out) - expected).abs() < 2e-6,
                    "case {case} {operation:?}: expected {expected}, got {}",
                    area(&out)
                );
                assert!(
                    out.iter()
                        .flat_map(|region| &region.outer)
                        .any(|edge| matches!(edge, CurveEdge2::Arc { .. }))
                        || out.is_empty()
                );
            }
        }
    }

    #[test]
    fn continuation_pair_subtracts_from_both_sides_of_a_third_host() {
        let base = rectangle(-2.0, -0.1, 0.30000000000000027, 0.1);
        let north = rectangle(-0.1, 0.0, 0.1, 2.0);
        let south_arc = CurveRegion2 {
            outer: vec![
                CurveEdge2::Line {
                    from: [-0.10000000000000009, 1.3471114790620887e-16],
                    to: [0.09999999999999998, 1.1021821192326179e-16],
                },
                CurveEdge2::Arc {
                    center: [1.0, 0.0],
                    radius: 0.9,
                    start_angle: std::f64::consts::PI,
                    sweep_angle: std::f64::consts::FRAC_PI_2,
                },
                CurveEdge2::Line {
                    from: [0.9999999999999999, -0.9],
                    to: [0.9999999999999998, -1.1],
                },
                CurveEdge2::Arc {
                    center: [1.0, 0.0],
                    radius: 1.1,
                    start_angle: 3.0 * std::f64::consts::FRAC_PI_2,
                    sweep_angle: -std::f64::consts::FRAC_PI_2,
                },
            ],
            holes: Vec::new(),
        };
        let out = boolean_curved_regions(
            &[base],
            &[north, south_arc],
            PlanarBooleanOp::Subtraction,
            1e-6,
        )
        .unwrap();
        assert_eq!(out.len(), 2);
        assert!(out
            .iter()
            .any(|region| region.outer.iter().any(|edge| edge.point(0.5).x < -1.0)));
        let original = rectangle(-2.0, -0.1, 0.0, 0.1);
        let intersections: Vec<_> = out
            .iter()
            .map(|region| {
                boolean_curved_regions(
                    &[region.clone()],
                    &[original.clone()],
                    PlanarBooleanOp::Intersection,
                    1e-6,
                )
                .unwrap()
            })
            .collect();
        assert_eq!(
            intersections
                .iter()
                .filter(|regions| !regions.is_empty())
                .count(),
            1
        );
        for region in out {
            boolean_curved_regions(
                &[region],
                &[original.clone()],
                PlanarBooleanOp::Subtraction,
                1e-6,
            )
            .unwrap();
        }
    }

    #[test]
    fn subtracting_concentric_disk_keeps_an_exact_circular_hole() {
        let disk = |radius: f64| CurveRegion2 {
            outer: vec![
                CurveEdge2::Arc {
                    center: [0.0, 0.0],
                    radius,
                    start_angle: 0.0,
                    sweep_angle: -std::f64::consts::PI,
                },
                CurveEdge2::Arc {
                    center: [0.0, 0.0],
                    radius,
                    start_angle: -std::f64::consts::PI,
                    sweep_angle: -std::f64::consts::PI,
                },
            ],
            holes: Vec::new(),
        };
        let out = boolean_curved_regions(
            &[disk(2.0)],
            &[disk(1.0)],
            PlanarBooleanOp::Subtraction,
            1e-7,
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].holes.len(), 1);
        assert!((area(&out) - 3.0 * std::f64::consts::PI).abs() < 1e-7);
        assert!(out[0].holes[0]
            .iter()
            .all(|edge| matches!(edge, CurveEdge2::Arc { .. })));
    }

    #[test]
    fn intersecting_disks_keep_exact_circle_arcs_and_lens_area() {
        let disk = |cx: f64| CurveRegion2 {
            outer: vec![
                CurveEdge2::Arc {
                    center: [cx, 0.0],
                    radius: 2.0,
                    start_angle: 0.0,
                    sweep_angle: -std::f64::consts::PI,
                },
                CurveEdge2::Arc {
                    center: [cx, 0.0],
                    radius: 2.0,
                    start_angle: -std::f64::consts::PI,
                    sweep_angle: -std::f64::consts::PI,
                },
            ],
            holes: Vec::new(),
        };
        let out = boolean_curved_regions(
            &[disk(-1.0)],
            &[disk(1.0)],
            PlanarBooleanOp::Intersection,
            1e-7,
        )
        .unwrap();
        let expected = 8.0 * (0.5_f64).acos() - 2.0 * 3.0_f64.sqrt();
        assert_eq!(out.len(), 1);
        assert!(out[0]
            .outer
            .iter()
            .all(|edge| matches!(edge, CurveEdge2::Arc { .. })));
        assert!((area(&out) - expected).abs() < 1e-7);
    }

    #[test]
    fn disjoint_output_has_canonical_region_and_ring_order() {
        let left = rectangle(-3.0, -1.0, -2.0, 1.0);
        let right = rectangle(2.0, -1.0, 3.0, 1.0);
        let first = boolean_curved_regions(
            &[right.clone(), left.clone()],
            &[],
            PlanarBooleanOp::Union,
            1e-7,
        )
        .unwrap();
        let second =
            boolean_curved_regions(&[left, right], &[], PlanarBooleanOp::Union, 1e-7).unwrap();
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].outer[0].point(0.0).x, -3.0);
        assert_eq!(first[1].outer[0].point(0.0).x, 2.0);
    }

    #[test]
    fn translated_and_rotated_circle_lenses_match_analytic_area() {
        let disk = |center: [f64; 2], angle: f64| CurveRegion2 {
            outer: vec![
                CurveEdge2::Arc {
                    center,
                    radius: 2.0,
                    start_angle: angle,
                    sweep_angle: -std::f64::consts::PI,
                },
                CurveEdge2::Arc {
                    center,
                    radius: 2.0,
                    start_angle: angle - std::f64::consts::PI,
                    sweep_angle: -std::f64::consts::PI,
                },
            ],
            holes: Vec::new(),
        };
        for index in 0..80 {
            let angle = index as f64 * 0.783_124;
            let distance = 0.25 + (index % 31) as f64 * 0.115;
            let centre = [
                3.0 + (index % 7) as f64 * 0.23,
                -4.0 + (index % 9) as f64 * 0.31,
            ];
            let other = [
                centre[0] + distance * angle.cos(),
                centre[1] + distance * angle.sin(),
            ];
            let actual = boolean_curved_regions(
                &[disk(centre, angle / 3.0)],
                &[disk(other, angle / 5.0)],
                PlanarBooleanOp::Intersection,
                1e-7,
            )
            .unwrap();
            let expected = 8.0 * (distance / 4.0).acos()
                - 0.5 * distance * (16.0 - distance * distance).sqrt();
            assert_eq!(actual.len(), 1, "case {index}");
            assert!(
                (area(&actual) - expected).abs() < 1e-6,
                "case {index}: expected {expected}, got {}",
                area(&actual)
            );
        }
    }
}
