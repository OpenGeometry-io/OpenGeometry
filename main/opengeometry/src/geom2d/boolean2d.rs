use super::poly2d::{
    cross2, dedupe_ring, dot2, point_on_edge_t, ring_inside2, segment_cross_t2, signed_area2,
    winding_number2, Edge2, Pt2,
};

pub(crate) struct RingRegion {
    pub(crate) outer: Vec<Pt2>,
    pub(crate) holes: Vec<Vec<Pt2>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlanarBooleanOp {
    Union,
    Intersection,
    Subtraction,
}

pub(crate) fn boolean_oriented_regions(
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

pub(crate) fn regions_from_edges_by(
    edges: &[Edge2],
    eps: f64,
    filled: impl Fn(Pt2) -> bool,
) -> Vec<RingRegion> {
    if edges.is_empty() {
        return Vec::new();
    }
    let split = subdivided_edges(edges, eps);
    let mut boundary = boundary_edges(&split, eps, filled);

    let mut seen: std::collections::HashSet<((i64, i64), (i64, i64))> =
        std::collections::HashSet::new();
    boundary.retain(|&(a, b)| seen.insert((quantized_key(a, eps), quantized_key(b, eps))));
    if boundary.is_empty() {
        return Vec::new();
    }

    let raw_loops = boundary_loops(&boundary, eps);
    nested_regions(raw_loops, eps)
}

fn subdivided_edges(edges: &[Edge2], eps: f64) -> Vec<Edge2> {
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
    split
}

fn boundary_edges(split: &[Edge2], eps: f64, filled: impl Fn(Pt2) -> bool) -> Vec<Edge2> {
    let mut boundary: Vec<Edge2> = Vec::new();
    for &(a, b) in split {
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
    boundary
}

fn quantized_key(point: Pt2, eps: f64) -> (i64, i64) {
    let step = (2.0 * eps).max(f64::EPSILON);
    (
        (point.x / step).round() as i64,
        (point.z / step).round() as i64,
    )
}

fn boundary_loops(boundary: &[Edge2], eps: f64) -> Vec<Vec<Pt2>> {
    let mut out_edges: std::collections::HashMap<(i64, i64), Vec<usize>> =
        std::collections::HashMap::new();
    for (idx, &(a, _)) in boundary.iter().enumerate() {
        out_edges
            .entry(quantized_key(a, eps))
            .or_default()
            .push(idx);
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
            for &c in out_edges
                .get(&quantized_key(b, eps))
                .map(|v| v.as_slice())
                .unwrap_or(&[])
            {
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
    raw_loops
}

fn nested_regions(raw_loops: Vec<Vec<Pt2>>, eps: f64) -> Vec<RingRegion> {
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
