//! Generic 2D nonzero-winding boolean: self-intersection resolution + multi-contour union.

use crate::geometry::poly2d::*;

/// Decompose a (possibly self-intersecting) closed ring into simple loops by
/// splitting at every self-crossing (stack method). Port of `decomposeSelfIntersections`.
/// The `HashMap` here is keyed by integer node id and only ever LOOKED UP (never
/// iterated), so it is deterministic.
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
    let mut next_id = n; // 0..n-1 = original vertices

    for i in 0..n {
        for j in (i + 1)..n {
            if j == (i + 1) % n || i == (j + 1) % n {
                continue; // adjacent edges share a vertex
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

    // Augmented cyclic node list: each original vertex, then its edge's crossings in order.
    let mut aug: Vec<(usize, Pt2)> = Vec::new();
    for i in 0..n {
        aug.push((i, ring[i]));
        per_edge[i].sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        for hit in &per_edge[i] {
            aug.push((hit.id, hit.point));
        }
    }

    // Stack decomposition: revisiting a node id pops the simple loop between.
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

/// A point just off the midpoint of `loop`'s first usable edge, on the given side.
/// Port of `edgeProbe2`.
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

/// A nonzero-winding region: outer boundary (CCW) + inner-void holes (CW).
pub struct RingRegion {
    pub outer: Vec<Pt2>,
    pub holes: Vec<Vec<Pt2>>,
}

/// Resolve a self-intersecting offset ring into the boundary of its nonzero-winding
/// region. Port of `resolveRingNonzero`. Returns `None` only if nothing fillable remains.
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
            continue; // interior edge of the fill — not a boundary
        }
        let area = signed_area2(&l); // >0 means the +1 (left) side is the interior
        let encloses_left = area > 0.0;
        let is_outer = encloses_left == filled_left;
        let mut ccw = l.clone();
        if signed_area2(&ccw) < 0.0 {
            ccw.reverse();
        }
        if is_outer {
            outer_rings.push(ccw);
        } else {
            ccw.reverse(); // hole CW
            hole_rings.push(ccw);
        }
    }

    if outer_rings.is_empty() {
        return None;
    }
    // One connected mass: take the largest outer ring; holes are those nested in it.
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

// ── Multi-contour nonzero-winding union (overlapping-band merge) ──────────────
// Union a set of CCW polygons (per-segment offset bands) into their filled-region
// boundary by the NONZERO-WINDING rule: build the planar arrangement (split every
// edge at all crossings + vertex-touches), keep only sub-edges that separate filled
// (winding ≠ 0) from empty, orient them filled-on-LEFT, and stitch into loops. Where
// bands OVERLAP (a self-crossing) the shared interior edges have filled on both
// sides → dropped, so the overlap merges into one clean region. Pure 2D, no CSG —
// the crossing cleanup the single-ring offset cannot do.

pub fn union_polygons_nonzero(polys: &[Vec<Pt2>], eps: f64) -> Vec<RingRegion> {
    // 1. directed edges from all (CCW) polygons + the vertex set for T-touch splits.
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
    if edges.is_empty() {
        return Vec::new();
    }
    let verts: Vec<Pt2> = edges.iter().map(|e| e.0).collect();

    // 2. split every edge at all crossings and on-edge vertices.
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

    // 3. keep sub-edges with filled on exactly one side; orient filled-on-LEFT.
    let winding_at = |p: Pt2| -> i32 { polys.iter().map(|poly| winding_number2(p, poly)).sum() };
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
        let left = winding_at(Pt2::new(mx + nx * probe, mz + nz * probe)) != 0;
        let right = winding_at(Pt2::new(mx - nx * probe, mz - nz * probe)) != 0;
        if left == right {
            continue; // interior or exterior — not a boundary
        }
        if left {
            boundary.push((a, b));
        } else {
            boundary.push((b, a));
        }
    }
    if boundary.is_empty() {
        return Vec::new();
    }

    // 4. stitch into loops; at each vertex take the next outgoing edge by smallest
    //    clockwise turn from the reverse of the incoming edge (standard face trace).
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
            let back = Pt2::new(a.x - b.x, a.z - b.z); // reverse incoming, from b toward a
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
                    cw += 2.0 * std::f64::consts::PI; // exclude the straight-back edge unless forced
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

    // 5. classify loops (CCW = outer / CW = hole) and nest holes into their outer.
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
