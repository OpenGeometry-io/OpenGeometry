//! Generic 2D polyline offset: miter/bevel joins, mitered bands, offset outline.

use openmaths::Vector3;
use crate::geometry::poly2d::*;
use crate::geometry::boolean2d::*;

/// Whether a centreline is (or should be treated as) a closed loop: the caller's
/// `closed` flag OR a first≈last coincidence. Returns the working points (closing
/// duplicate stripped) and the resolved `closed` flag.
pub fn resolve_closed(points: &[Pt2], closed: bool, eps: f64) -> (Vec<Pt2>, bool) {
    let mut pts = points.to_vec();
    let mut is_closed = closed;
    if pts.len() >= 2 {
        let first = pts[0];
        let last = pts[pts.len() - 1];
        if (first.x - last.x).hypot(first.z - last.z) < eps {
            is_closed = true;
            pts.pop();
        }
    }
    (pts, is_closed)
}

/// Drop centreline vertices that create a segment shorter than `min_length`.
pub fn simplify_polyline(centreline: &[Vector3], min_length: f64, closed: bool) -> Vec<Vector3> {
    if centreline.len() < 3 || min_length <= 0.0 {
        return centreline.to_vec();
    }
    let near = |a: &Vector3, b: &Vector3| (a.x - b.x).hypot(a.z - b.z);

    let mut pts = centreline.to_vec();
    let mut had_closing_dup = false;
    if closed && pts.len() >= 2 && near(&pts[0], &pts[pts.len() - 1]) < DEFAULT_EPS {
        pts.pop();
        had_closing_dup = true;
    }

    let mut kept: Vec<Vector3> = vec![pts[0]];
    for current in pts.iter().skip(1) {
        if near(current, kept.last().unwrap()) >= min_length {
            kept.push(*current);
        }
    }

    if !closed {
        // Keep the true endpoint (overall length matters): replace a short tail, else append.
        let endpoint = pts[pts.len() - 1];
        let last = *kept.last().unwrap();
        if near(&endpoint, &last) > DEFAULT_EPS {
            if kept.len() >= 2 && near(&endpoint, &last) < min_length {
                *kept.last_mut().unwrap() = endpoint;
            } else {
                kept.push(endpoint);
            }
        }
    } else {
        // Drop a too-short closing segment (last kept → first).
        if kept.len() > 3 && near(&kept[0], kept.last().unwrap()) < min_length {
            kept.pop();
        }
        if had_closing_dup {
            let first = kept[0];
            kept.push(Vector3::new(first.x, first.y, first.z));
        }
    }

    // Never over-collapse below a buildable polyline.
    let min_kept = if closed {
        if had_closing_dup {
            4
        } else {
            3
        }
    } else {
        2
    };
    if kept.len() < min_kept {
        return centreline.to_vec();
    }
    kept
}

/// A stroked region as a clean outline: outer ring + inner-ring holes (XZ at base_y).
pub struct Outline2D {
    pub outer: Vec<Vector3>,
    pub holes: Vec<Vec<Vector3>>,
}

/// Per-segment MITERED bands (each CCW). Convex corners carry the offset-edge miter
/// (a shared apex with the neighbour, so the convex corner is FILLED — no square-cap
/// notch); concave corners carry the segment's own perpendicular cap and let the
/// neighbour band overlap to fill the fold. This is the same per-corner join a
/// simple centreline uses, so a self-crossing centreline's vertices join identically.
/// `miter_limit` ∞ keeps convex corners full miters; spikes are bevelled post-union.
pub fn mitered_offset_bands(
    pts: &[Pt2],
    closed: bool,
    half_width: f64,
    miter_limit: f64,
    eps: f64,
) -> Vec<Vec<Pt2>> {
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let seg_count = if closed { n } else { n - 1 };
    let mut dir: Vec<Pt2> = Vec::with_capacity(seg_count);
    let mut left_normal: Vec<Pt2> = Vec::with_capacity(seg_count);
    for i in 0..seg_count {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let length = (b.x - a.x).hypot(b.z - a.z);
        if length < eps {
            return Vec::new();
        }
        dir.push(Pt2::new((b.x - a.x) / length, (b.z - a.z) / length));
        left_normal.push(Pt2::new(-(b.z - a.z) / length, (b.x - a.x) / length));
    }
    let cap = |vi: usize, seg: usize, side: f64| -> Pt2 {
        Pt2::new(
            pts[vi].x + side * half_width * left_normal[seg].x,
            pts[vi].z + side * half_width * left_normal[seg].z,
        )
    };
    let corner = |vi: usize, in_seg: usize, out_seg: usize, side: f64, this_seg: usize| -> Pt2 {
        let turn = cross2(dir[in_seg], dir[out_seg]);
        if side * turn >= 0.0 {
            return cap(vi, this_seg, side); // concave side → own cap, let overlap fill
        }
        let Some(meet) = intersect2(
            cap(vi, in_seg, side),
            dir[in_seg],
            cap(vi, out_seg, side),
            dir[out_seg],
            eps,
        ) else {
            return cap(vi, this_seg, side);
        };
        let cos_deflection = dot2(dir[in_seg], dir[out_seg]).clamp(-1.0, 1.0);
        let miter_ratio = 1.0 / (cos_deflection.acos() / 2.0).cos();
        if miter_ratio > miter_limit {
            return cap(vi, this_seg, side);
        }
        meet
    };

    let mut bands: Vec<Vec<Pt2>> = Vec::with_capacity(seg_count);
    for i in 0..seg_count {
        let start_vi = i;
        let end_vi = (i + 1) % n;
        let start_free = !closed && i == 0;
        let end_free = !closed && i == seg_count - 1;
        let prev_seg = (i + seg_count - 1) % seg_count;
        let next_seg = (i + 1) % seg_count;
        let mut quad: Vec<Pt2> = vec![
            if start_free { cap(start_vi, i, 1.0) } else { corner(start_vi, prev_seg, i, 1.0, i) },
            if end_free { cap(end_vi, i, 1.0) } else { corner(end_vi, i, next_seg, 1.0, i) },
            if end_free { cap(end_vi, i, -1.0) } else { corner(end_vi, i, next_seg, -1.0, i) },
            if start_free { cap(start_vi, i, -1.0) } else { corner(start_vi, prev_seg, i, -1.0, i) },
        ];
        if self_intersects2(&quad, eps) || signed_area2(&quad).abs() < eps {
            // Over-deep trim folded the quad — fall back to a plain rectangle band.
            quad = vec![
                cap(start_vi, i, 1.0),
                cap(end_vi, i, 1.0),
                cap(end_vi, i, -1.0),
                cap(start_vi, i, -1.0),
            ];
        }
        if signed_area2(&quad) < 0.0 {
            quad.reverse();
        }
        bands.push(quad);
    }
    bands
}

/// Replace sharp convex miter spikes on a 2D ring with a flat BEVEL (the chamfer
/// joining the two offset points), past `miter_limit` — the same threshold the
/// per-corner joins use. Concave vertices and corners within the limit are kept.
pub fn bevel_sharp_convex_pts(ring: &[Pt2], half_width: f64, miter_limit: f64, eps: f64) -> Vec<Pt2> {
    let count = ring.len();
    if count < 3 {
        return ring.to_vec();
    }
    let ring_is_ccw = signed_area2(ring) > 0.0;
    let mut out: Vec<Pt2> = Vec::with_capacity(count);
    for i in 0..count {
        let prev = ring[(i + count - 1) % count];
        let vertex = ring[i];
        let next = ring[(i + 1) % count];
        let in_dir = Pt2::new(vertex.x - prev.x, vertex.z - prev.z);
        let in_len = in_dir.x.hypot(in_dir.z);
        let out_dir = Pt2::new(next.x - vertex.x, next.z - vertex.z);
        let out_len = out_dir.x.hypot(out_dir.z);
        if in_len < eps || out_len < eps {
            out.push(vertex);
            continue;
        }
        let to_prev = Pt2::new(-in_dir.x / in_len, -in_dir.z / in_len);
        let to_next = Pt2::new(out_dir.x / out_len, out_dir.z / out_len);
        let turn = cross2(
            Pt2::new(in_dir.x / in_len, in_dir.z / in_len),
            Pt2::new(out_dir.x / out_len, out_dir.z / out_len),
        );
        let convex = if ring_is_ccw { turn > eps } else { turn < -eps };
        let interior_angle = dot2(to_prev, to_next).clamp(-1.0, 1.0).acos();
        let ratio = if interior_angle > eps {
            1.0 / (interior_angle / 2.0).sin()
        } else {
            f64::INFINITY
        };
        if !convex || ratio <= miter_limit {
            out.push(vertex);
            continue;
        }
        let mut trim = half_width / (interior_angle / 2.0).tan();
        trim = trim.min(0.5 * in_len).min(0.5 * out_len);
        if trim <= eps {
            out.push(vertex);
            continue;
        }
        out.push(Pt2::new(vertex.x + to_prev.x * trim, vertex.z + to_prev.z * trim));
        out.push(Pt2::new(vertex.x + to_next.x * trim, vertex.z + to_next.z * trim));
    }
    out
}

/// Boundary points contributed at a vertex on one side.
/// Convex side → miter (1 pt) within the limit, else a flat bevel (2 caps).
/// Concave side → inner trim (1 pt).
#[allow(clippy::too_many_arguments)]
pub fn join_points(
    vertex: Pt2,
    in_dir: Pt2,
    out_dir: Pt2,
    in_normal: Pt2,
    out_normal: Pt2,
    side: f64,
    half_width: f64,
    miter_limit: f64,
    eps: f64,
    forward: bool,
) -> Vec<Pt2> {
    let in_anchor = Pt2::new(
        vertex.x + side * half_width * in_normal.x,
        vertex.z + side * half_width * in_normal.z,
    );
    let out_anchor = Pt2::new(
        vertex.x + side * half_width * out_normal.x,
        vertex.z + side * half_width * out_normal.z,
    );
    let Some(corner) = intersect2(in_anchor, in_dir, out_anchor, out_dir, eps) else {
        return vec![in_anchor]; // collinear — anchors coincide
    };

    let turn = cross2(in_dir, out_dir);
    let side_convex = side * turn < 0.0;
    if !side_convex {
        return vec![corner]; // concave inner trim
    }

    let deflection = dot2(in_dir, out_dir).clamp(-1.0, 1.0).acos();
    let miter_ratio = 1.0 / (deflection / 2.0).cos();
    if miter_ratio <= miter_limit {
        return vec![corner]; // miter
    }
    if forward {
        vec![in_anchor, out_anchor]
    } else {
        vec![out_anchor, in_anchor]
    } // flat bevel
}

/// Offset a centreline into a clean outline polygon with miter/bevel joins.
/// Returns `None` if the outline (or, for a closed centreline, either ring) would
/// still self-intersect after nonzero-winding resolution.
pub fn offset_outline(
    centreline: &[Vector3],
    width: f64,
    closed: bool,
    miter_limit: f64,
    eps: f64,
) -> Option<Outline2D> {
    let half_width = width / 2.0;
    let base_y = centreline.first().map(|v| v.y).unwrap_or(0.0);

    let raw: Vec<Pt2> = centreline.iter().map(xz).collect();
    let (pts, closed) = resolve_closed(&raw, closed, eps);
    let n = pts.len();
    if n < 2 || (closed && n < 3) {
        return None;
    }

    let seg_count = if closed { n } else { n - 1 };
    let mut dir: Vec<Pt2> = Vec::with_capacity(seg_count);
    let mut left_normal: Vec<Pt2> = Vec::with_capacity(seg_count);
    for i in 0..seg_count {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let dx = b.x - a.x;
        let dz = b.z - a.z;
        let length = dx.hypot(dz);
        if length < eps {
            return None;
        }
        dir.push(Pt2::new(dx / length, dz / length));
        left_normal.push(Pt2::new(-dz / length, dx / length));
    }

    let points_at = |vi: usize, in_seg: usize, out_seg: usize, side: f64, forward: bool| -> Vec<Pt2> {
        join_points(
            pts[vi],
            dir[in_seg],
            dir[out_seg],
            left_normal[in_seg],
            left_normal[out_seg],
            side,
            half_width,
            miter_limit,
            eps,
            forward,
        )
    };
    let cap = |vi: usize, seg: usize, side: f64| -> Pt2 {
        Pt2::new(
            pts[vi].x + side * half_width * left_normal[seg].x,
            pts[vi].z + side * half_width * left_normal[seg].z,
        )
    };

    let to_v3 = |ring: Vec<Pt2>| -> Vec<Vector3> {
        ring.into_iter()
            .map(|p| Vector3::new(p.x, base_y, p.z))
            .collect()
    };

    if !closed {
        let mut left: Vec<Pt2> = vec![cap(0, 0, 1.0)];
        for vi in 1..=(n - 2) {
            left.extend(points_at(vi, vi - 1, vi, 1.0, true));
        }
        left.push(cap(n - 1, seg_count - 1, 1.0));

        let mut right: Vec<Pt2> = vec![cap(n - 1, seg_count - 1, -1.0)];
        for vi in (1..=(n - 2)).rev() {
            right.extend(points_at(vi, vi - 1, vi, -1.0, false));
        }
        right.push(cap(0, 0, -1.0));

        let mut combined = left;
        combined.extend(right);
        let ring = dedupe_ring(&combined, eps);
        if ring.len() < 3 {
            return None;
        }
        let resolved = resolve_ring_nonzero(&ring, eps)?;
        return Some(Outline2D {
            outer: to_v3(resolved.outer),
            holes: resolved.holes.into_iter().map(to_v3).collect(),
        });
    }

    // A simple closed centreline may not cross itself (e.g. a figure-8): its offset
    // can't be a single outer+holes region, so reject cleanly rather than emit a partial lobe.
    if self_intersects2(&pts, eps) {
        return None;
    }

    // Closed: two rings (offset +h and −h all the way round), each resolved by
    // nonzero winding. The larger-area region is the outer boundary; the smaller is
    // the interior void (a hole) — UNLESS the inner offset collapses or inverts (a
    // stroke as wide as its loop), in which case the stroke fills the region: SOLID, no hole.
    let ring_for_side = |side: f64| -> Vec<Pt2> {
        let mut out: Vec<Pt2> = Vec::new();
        for vi in 0..n {
            out.extend(points_at(vi, (vi + n - 1) % n, vi, side, true));
        }
        dedupe_ring(&out, eps)
    };

    // Keep only sides that produced a real (positive-area) region; the larger is outer.
    let mut regions: Vec<RingRegion> = [
        resolve_ring_nonzero(&ring_for_side(1.0), eps),
        resolve_ring_nonzero(&ring_for_side(-1.0), eps),
    ]
    .into_iter()
    .flatten()
    .filter(|region| signed_area2(&region.outer).abs() >= eps)
    .collect();
    if regions.is_empty() {
        return None; // both offset rings collapsed — genuinely unbuildable
    }
    regions.sort_by(|a, b| {
        signed_area2(&b.outer)
            .abs()
            .partial_cmp(&signed_area2(&a.outer).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let outer_region = regions.remove(0);

    // The smaller side is the interior void only if it nests cleanly inside the outer;
    // otherwise it collapsed/inverted → no hole (solid mass).
    let mut hole_rings: Vec<Vec<Pt2>> = Vec::new();
    if let Some(inner_region) = regions.into_iter().next() {
        if ring_inside2(&inner_region.outer, &outer_region.outer, eps) {
            hole_rings.push(inner_region.outer);
        }
    }
    hole_rings.extend(outer_region.holes);
    let holes: Vec<Vec<Pt2>> = hole_rings
        .into_iter()
        .filter(|hole| ring_inside2(hole, &outer_region.outer, eps))
        .collect();

    Some(Outline2D {
        outer: to_v3(outer_region.outer),
        holes: holes.into_iter().map(to_v3).collect(),
    })
}
