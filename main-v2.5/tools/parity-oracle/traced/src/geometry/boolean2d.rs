use crate::geometry::poly2d::*;

pub fn decompose_self_intersections(ring: &[Pt2], eps: f64) -> Vec<Vec<Pt2>> {
    let n = ring.len();
    if n < 3 {
        return Vec::new();
    }

    struct Hit {
        t: f64,
        point: Pt2,
        id: usize,
    }
    let mut per_edge: Vec<Vec<Hit>> = (0..n).map(|_| Vec::new()).collect();
    let mut interns: Vec<(Pt2, usize)> = Vec::new();
    let mut next_id = n;

    for i in 0..n {
        for j in (i + 1)..n {
            if j == (i + 1) % n || i == (j + 1) % n {
                continue;
            }
            if let Some((point, ta, tb)) =
                segment_cross_t2(ring[i], ring[(i + 1) % n], ring[j], ring[(j + 1) % n], eps)
            {
                let mut id = None;
                for (pt, existing) in &interns {
                    if (pt.x - point.x).hypot(pt.z - point.z) < 1.0e-6 {
                        id = Some(*existing);
                        break;
                    }
                }
                let id = id.unwrap_or_else(|| {
                    let assigned = next_id;
                    next_id += 1;
                    interns.push((point, assigned));
                    assigned
                });
                per_edge[i].push(Hit { t: ta, point, id });
                per_edge[j].push(Hit { t: tb, point, id });
            }
        }
    }

    let mut aug: Vec<(usize, Pt2)> = Vec::new();
    for i in 0..n {
        aug.push((i, ring[i]));
        per_edge[i].sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        for hit in &per_edge[i] {
            aug.push((hit.id, hit.point));
        }
    }

    let mut loops: Vec<Vec<Pt2>> = Vec::new();
    let mut path: Vec<(usize, Pt2)> = Vec::new();
    let mut index_of_id: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for node in &aug {
        if let Some(&at) = index_of_id.get(&node.0) {
            let loop_pts: Vec<Pt2> = path[at..].iter().map(|nd| nd.1).collect();
            if loop_pts.len() >= 3 {
                loops.push(loop_pts);
            }
            let mut m = path.len();
            while m > at + 1 {
                m -= 1;
                index_of_id.remove(&path[m].0);
            }
            path.truncate(at + 1);
        } else {
            index_of_id.insert(node.0, path.len());
            path.push(*node);
        }
    }
    if path.len() >= 3 {
        loops.push(path.iter().map(|nd| nd.1).collect());
    }
    loops
}

pub fn edge_probe2(ring: &[Pt2], side: f64) -> Option<Pt2> {
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        let ex = b.x - a.x;
        let ez = b.z - a.z;
        let len = ex.hypot(ez);
        if len < 1.0e-9 {
            continue;
        }
        let nx = -ez / len;
        let nz = ex / len;
        let d = (len * 0.25).min(1.0e-3);
        let mx = (a.x + b.x) / 2.0;
        let mz = (a.z + b.z) / 2.0;
        return Some(Pt2::new(mx + side * nx * d, mz + side * nz * d));
    }
    None
}

pub struct RingRegion {
    pub outer: Vec<Pt2>,
    pub holes: Vec<Vec<Pt2>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanarBooleanOp {
    Union,
    Intersection,
    Subtraction,
}

pub fn resolve_ring_nonzero(input: &[Pt2], eps: f64) -> Option<RingRegion> {
    let ring = dedupe_ring(input, eps);
    if ring.len() < 3 {
        return None;
    }
    if !self_intersects2(&ring, eps) {
        let outer = if signed_area2(&ring) < 0.0 {
            ring.iter().rev().copied().collect()
        } else {
            ring.clone()
        };
        return Some(RingRegion {
            outer,
            holes: Vec::new(),
        });
    }

    let loops: Vec<Vec<Pt2>> = decompose_self_intersections(&ring, eps)
        .into_iter()
        .map(|l| dedupe_ring(&l, eps))
        .filter(|l| l.len() >= 3 && signed_area2(l).abs() > eps)
        .collect();

    let mut outer_rings: Vec<Vec<Pt2>> = Vec::new();
    let mut hole_rings: Vec<Vec<Pt2>> = Vec::new();
    for l in loops {
        let (Some(p_in), Some(p_out)) = (edge_probe2(&l, 1.0), edge_probe2(&l, -1.0)) else {
            continue;
        };
        let filled_left = winding_number2(p_in, &ring) != 0;
        let filled_right = winding_number2(p_out, &ring) != 0;
        if filled_left == filled_right {
            continue;
        }
        let area = signed_area2(&l);
        let encloses_left = area > 0.0;
        let is_outer = encloses_left == filled_left;
        let mut ccw = l.clone();
        if signed_area2(&ccw) < 0.0 {
            ccw.reverse();
        }
        if is_outer {
            outer_rings.push(ccw);
        } else {
            ccw.reverse();
            hole_rings.push(ccw);
        }
    }

    if outer_rings.is_empty() {
        return None;
    }

    outer_rings.sort_by(|a, b| {
        signed_area2(b)
            .abs()
            .partial_cmp(&signed_area2(a).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let outer = outer_rings.into_iter().next().unwrap();
    let holes = hole_rings
        .into_iter()
        .filter(|hole| ring_inside2(hole, &outer, eps))
        .collect();
    Some(RingRegion { outer, holes })
}

pub fn union_polygons_nonzero(polys: &[Vec<Pt2>], eps: f64) -> Vec<RingRegion> {
    winding_regions(polys, eps, |winding| winding != 0)
}

pub fn resolve_polygons_positive(polys: &[Vec<Pt2>], eps: f64) -> Vec<RingRegion> {
    winding_regions(polys, eps, |winding| winding > 0)
}

pub fn boolean_oriented_regions(
    a: &[Vec<Pt2>],
    b: &[Vec<Pt2>],
    operation: PlanarBooleanOp,
    eps: f64,
) -> Vec<RingRegion> {
    let mut contours = Vec::with_capacity(a.len() + b.len());
    contours.extend_from_slice(a);
    contours.extend_from_slice(b);
    winding_regions_by(&contours, eps, |point| {
        let in_a = a
            .iter()
            .map(|ring| winding_number2(point, ring))
            .sum::<i32>()
            != 0;
        let in_b = b
            .iter()
            .map(|ring| winding_number2(point, ring))
            .sum::<i32>()
            != 0;
        match operation {
            PlanarBooleanOp::Union => in_a || in_b,
            PlanarBooleanOp::Intersection => in_a && in_b,
            PlanarBooleanOp::Subtraction => in_a && !in_b,
        }
    })
}

fn winding_regions(polys: &[Vec<Pt2>], eps: f64, filled: impl Fn(i32) -> bool) -> Vec<RingRegion> {
    winding_regions_by(polys, eps, |point| {
        filled(polys.iter().map(|poly| winding_number2(point, poly)).sum())
    })
}

fn winding_regions_by(
    polys: &[Vec<Pt2>],
    eps: f64,
    filled: impl Fn(Pt2) -> bool,
) -> Vec<RingRegion> {
    let mut edges: Vec<Edge2> = Vec::new();
    for poly in polys {
        let m = poly.len();
        for i in 0..m {
            let a = poly[i];
            let b = poly[(i + 1) % m];
            if (a.x - b.x).hypot(a.z - b.z) > eps {
                edges.push((a, b));
            }
        }
    }
    regions_from_edges_by(&edges, eps, filled)
}

pub fn regions_from_edges_by(
    edges: &[Edge2],
    eps: f64,
    filled: impl Fn(Pt2) -> bool,
) -> Vec<RingRegion> {
    if edges.is_empty() {
        return Vec::new();
    }
    let verts: Vec<Pt2> = edges.iter().flat_map(|&(a, b)| [a, b]).collect();

    let mut split: Vec<Edge2> = Vec::new();
    for (i, &(ea, eb)) in edges.iter().enumerate() {
        let rx = eb.x - ea.x;
        let rz = eb.z - ea.z;
        let mut ts: Vec<f64> = vec![0.0, 1.0];
        for (j, &(fa, fb)) in edges.iter().enumerate() {
            if i != j {
                if let Some((_, ta, _)) = segment_cross_t2(ea, eb, fa, fb, eps) {
                    ts.push(ta);
                }
            }
        }
        for &v in &verts {
            if let Some(t) = point_on_edge_t(ea, eb, v, eps) {
                ts.push(t);
            }
        }
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut prev = f64::NEG_INFINITY;
        let mut clean: Vec<f64> = Vec::new();
        for t in ts {
            if t - prev > eps {
                clean.push(t);
                prev = t;
            }
        }
        for w in clean.windows(2) {
            let p0 = Pt2::new(ea.x + w[0] * rx, ea.z + w[0] * rz);
            let p1 = Pt2::new(ea.x + w[1] * rx, ea.z + w[1] * rz);
            if (p0.x - p1.x).hypot(p0.z - p1.z) > eps {
                split.push((p0, p1));
            }
        }
    }

    let mut boundary: Vec<Edge2> = Vec::new();
    for &(a, b) in &split {
        let dx = b.x - a.x;
        let dz = b.z - a.z;
        let len = dx.hypot(dz);
        if len < eps {
            continue;
        }
        let probe = (len * 0.25).min(1.0e-4);
        let nx = -dz / len;
        let nz = dx / len;
        let mx = (a.x + b.x) / 2.0;
        let mz = (a.z + b.z) / 2.0;
        let left = filled(Pt2::new(mx + nx * probe, mz + nz * probe));
        let right = filled(Pt2::new(mx - nx * probe, mz - nz * probe));
        if left == right {
            continue;
        }
        if left {
            boundary.push((a, b));
        } else {
            boundary.push((b, a));
        }
    }

    let mut seen: std::collections::HashSet<((i64, i64), (i64, i64))> =
        std::collections::HashSet::new();
    let qkey = |point: Pt2| {
        let step = (2.0 * eps).max(f64::EPSILON);
        (
            (point.x / step).round() as i64,
            (point.z / step).round() as i64,
        )
    };
    boundary.retain(|&(a, b)| seen.insert((qkey(a), qkey(b))));
    if boundary.is_empty() {
        return Vec::new();
    }

    let mut out_edges: std::collections::HashMap<(i64, i64), Vec<usize>> =
        std::collections::HashMap::new();
    for (idx, &(a, _)) in boundary.iter().enumerate() {
        out_edges.entry(qkey(a)).or_default().push(idx);
    }
    let mut used = vec![false; boundary.len()];
    let mut raw_loops: Vec<Vec<Pt2>> = Vec::new();
    for start in 0..boundary.len() {
        if used[start] {
            continue;
        }
        let mut loop_pts: Vec<Pt2> = Vec::new();
        let mut cur = start;
        let mut guard = 0;
        while !used[cur] && guard <= boundary.len() {
            guard += 1;
            used[cur] = true;
            let (a, b) = boundary[cur];
            loop_pts.push(a);
            let back = Pt2::new(a.x - b.x, a.z - b.z);
            let mut best: Option<usize> = None;
            let mut best_cw = f64::INFINITY;
            for &c in out_edges.get(&qkey(b)).map(|v| v.as_slice()).unwrap_or(&[]) {
                if used[c] {
                    continue;
                }
                let (ca, cb) = boundary[c];
                let out_dir = Pt2::new(cb.x - ca.x, cb.z - ca.z);
                let mut cw = -cross2(back, out_dir).atan2(dot2(back, out_dir));
                if cw < eps {
                    cw += 2.0 * std::f64::consts::PI;
                }
                if cw < best_cw {
                    best_cw = cw;
                    best = Some(c);
                }
            }
            match best {
                Some(next) => cur = next,
                None => break,
            }
        }
        if loop_pts.len() >= 3 {
            raw_loops.push(loop_pts);
        }
    }

    let mut regions: Vec<RingRegion> = Vec::new();
    let mut holes: Vec<Vec<Pt2>> = Vec::new();
    for raw in raw_loops {
        let ring = dedupe_ring(&raw, eps);
        if ring.len() < 3 || signed_area2(&ring).abs() < eps {
            continue;
        }
        if signed_area2(&ring) > 0.0 {
            regions.push(RingRegion {
                outer: ring,
                holes: Vec::new(),
            });
        } else {
            holes.push(ring);
        }
    }
    for hole in holes {
        if let Some(region) = regions
            .iter_mut()
            .find(|region| ring_inside2(&hole, &region.outer, eps))
        {
            region.holes.push(hole);
        }
    }
    regions
}
