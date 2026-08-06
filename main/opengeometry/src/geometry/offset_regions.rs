//! Fixed-width polyline offset: centreline + width -> stroked region(s), plus a
//! group offset that merges several overlapping strokes into one. Built on the
//! generic `geometry` 2D toolkit. Domain-neutral (walls, roads, pipe routes,
//! hatching, font strokes, etc.).

use crate::geometry::boolean2d::*;
use crate::geometry::offset2d::*;
use crate::geometry::poly2d::*;
use openmaths::Vector3;
use wasm_bindgen::prelude::*;

/// One stroked region: a CW outer ring + its CCW inner-void holes (canonical).
pub type OffsetRegion = (Vec<Vector3>, Vec<Vec<Vector3>>);

/// Offset a centreline by ±half-width into one or more stroked regions, via the
/// deterministic analytic offset. A simple centreline (open, L, closed loop,
/// reflex, tight) yields ONE region from the single-ring offset. A self-crossing
/// CLOSED centreline (e.g. a figure-8) instead UNIONS its per-segment bands by
/// nonzero winding, so the overlapping strokes at the crossing MERGE into one
/// clean region (no internal edges) — the cleanup a single-ring offset can't do.
/// Each region is `normalize_winding` (CW outer / CCW holes) + `canonical_ring`.
/// Empty vec = nothing buildable.
pub fn offset_polyline_regions(
    centreline: &[Vector3],
    width: f64,
    closed: bool,
    miter_limit: f64,
    eps: f64,
) -> Vec<OffsetRegion> {
    if width <= 0.0 || !width.is_finite() || centreline.len() < 2 {
        return Vec::new();
    }
    let simplified = simplify_polyline(centreline, width / 2.0, closed);
    let base_y = simplified.first().map(|v| v.y).unwrap_or(0.0);

    let raw: Vec<Pt2> = simplified.iter().map(xz).collect();
    let (pts, is_closed) = resolve_closed(&raw, closed, eps);

    // Self-crossing closed centreline (figure-8 etc.): union the per-segment MITERED
    // bands by nonzero winding so (a) every vertex joins with a miter like a normal
    // corner (no square-cap notch), (b) the overlapping strokes at each crossing
    // merge into one region, then bevel any convex spike past the miter limit — the
    // same corner treatment every other corner gets.
    if is_closed && self_intersects2(&pts, eps) {
        let half_width = width / 2.0;
        let bands = mitered_offset_bands(&pts, true, half_width, f64::INFINITY, eps);
        return union_polygons_nonzero(&bands, eps)
            .into_iter()
            .map(|region| {
                let outer = bevel_sharp_convex_pts(&region.outer, half_width, miter_limit, eps);
                finalize_region(&outer, &region.holes, base_y)
            })
            .collect();
    }

    // Simple centrelines: the single-ring analytic offset (one region, or none if degenerate).
    match offset_outline(&simplified, width, is_closed, miter_limit, eps) {
        Some(outline) => {
            let outer = canonical_ring(&normalize_winding(&outline.outer, Winding::Cw));
            let holes = outline
                .holes
                .iter()
                .map(|hole| canonical_ring(&normalize_winding(hole, Winding::Ccw)))
                .collect();
            vec![(outer, holes)]
        }
        None => Vec::new(),
    }
}

/// Convert a resolved 2D region (outer + holes, in Pt2) to an `OffsetRegion`:
/// lift to `base_y`, normalise winding (CW outer / CCW holes), canonicalise.
pub fn finalize_region(outer: &[Pt2], hole_rings: &[Vec<Pt2>], base_y: f64) -> OffsetRegion {
    let to_v3 = |ring: &[Pt2]| -> Vec<Vector3> {
        ring.iter()
            .map(|p| Vector3::new(p.x, base_y, p.z))
            .collect()
    };
    let outer = canonical_ring(&normalize_winding(&to_v3(outer), Winding::Cw));
    let holes = hole_rings
        .iter()
        .map(|hole| canonical_ring(&normalize_winding(&to_v3(hole), Winding::Ccw)))
        .collect();
    (outer, holes)
}

/// Segments per stadium end-cap semicircle. Caps are CIRCUMSCRIBED tangent
/// fans, so the forbidden zone always CONTAINS the exact clearance disc — the
/// envelope can under-claim by the tessellation sliver (≈0.9 % radially at 12
/// segments) but can never claim a point closer to an edge than its distance.
const STADIUM_CAP_SEGMENTS: usize = 12;

/// The forbidden zone of one edge: every point within `distance` of the
/// segment a→b — a stadium (rectangle + two circumscribed end-cap fans),
/// wound CW so it subtracts under the positive-winding rule.
fn forbidden_stadium(a: Pt2, b: Pt2, distance: f64) -> Vec<Pt2> {
    let length = (b.x - a.x).hypot(b.z - a.z);
    let u = Pt2::new((b.x - a.x) / length, (b.z - a.z) / length);
    let n = Pt2::new(-u.z, u.x);

    let cap = |centre: Pt2, from_angle: f64| -> Vec<Pt2> {
        // Tangent fan sweeping a semicircle CLOCKWISE from `from_angle`:
        // vertices at the tangent-line intersections, radius d / cos(Δ/2).
        let delta = std::f64::consts::PI / STADIUM_CAP_SEGMENTS as f64;
        let radius = distance / (delta / 2.0).cos();
        (0..STADIUM_CAP_SEGMENTS)
            .map(|j| {
                let angle = from_angle - (j as f64 + 0.5) * delta;
                Pt2::new(
                    centre.x + radius * angle.cos(),
                    centre.z + radius * angle.sin(),
                )
            })
            .collect()
    };

    let normal_angle = n.z.atan2(n.x);
    let mut ring: Vec<Pt2> = Vec::with_capacity(4 + 2 * STADIUM_CAP_SEGMENTS);
    ring.push(Pt2::new(a.x + distance * n.x, a.z + distance * n.z));
    ring.push(Pt2::new(b.x + distance * n.x, b.z + distance * n.z));
    ring.extend(cap(b, normal_angle)); // sweep through +u, beyond b
    ring.push(Pt2::new(b.x - distance * n.x, b.z - distance * n.z));
    ring.push(Pt2::new(a.x - distance * n.x, a.z - distance * n.z));
    ring.extend(cap(a, normal_angle + std::f64::consts::PI)); // through -u, beyond a
    ring // n = u rotated +90°, so this traversal is CW (winding −1)
}

/// Inset a CLOSED ring inward, one distance per edge (edge i = ring[i] →
/// ring[i+1], after stripping an optional closing duplicate). The result is
/// the CLEARANCE-EXACT buildable region: the ring's interior minus, per edge,
/// every point within that edge's distance of the edge SEGMENT (not just its
/// line) — resolved as a positive-winding clip of the ring against per-edge
/// forbidden stadiums. Corners therefore become circumscribed clearance arcs
/// where distances differ (no miters, no bevels — both only approximate the
/// arc, and a bevel would violate the clearance), distant edges are honoured
/// across notches, and an over-deep inset collapses to nothing instead of
/// inverting. Returns zero or more disjoint regions (a deep inset can split a
/// waisted ring into separate lobes), each CW-outer / CCW-holes, canonicalised
/// — the same output contract as `offset_polyline_regions`. `Ok(vec![])` = the
/// inset collapsed or the input ring is degenerate; `Err` = malformed
/// ARGUMENTS (distance count mismatch, negative / non-finite distance).
pub fn offset_ring_variable(
    ring: &[Vector3],
    distances: &[f64],
    eps: f64,
) -> Result<Vec<OffsetRegion>, String> {
    let base_y = ring.first().map(|v| v.y).unwrap_or(0.0);
    let raw: Vec<Pt2> = ring.iter().map(xz).collect();
    let (pts, _) = resolve_closed(&raw, true, eps);
    let n = pts.len();

    if distances.len() != n {
        return Err(format!(
            "Expected one distance per ring edge (edge i = ring[i] -> ring[i+1]): the ring has {} edges but {} distances were given",
            n,
            distances.len()
        ));
    }
    for (i, d) in distances.iter().enumerate() {
        if !d.is_finite() || *d < 0.0 {
            return Err(format!(
                "distances[{}] = {} is invalid: every per-edge distance must be a finite value >= 0",
                i, d
            ));
        }
    }

    // Drop zero-length edges together with their distance slot, so the
    // per-edge pairing stays aligned. (`simplify_polyline` is deliberately not
    // used: its width-based vertex dropping would desync `distances`, and a
    // collinear vertex splitting one frontage into different setbacks is a
    // feature, not noise.)
    let mut pts2: Vec<Pt2> = Vec::with_capacity(n);
    let mut dist2: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        if (a.x - b.x).hypot(a.z - b.z) < eps {
            continue;
        }
        pts2.push(a);
        dist2.push(distances[i]);
    }

    let m = pts2.len();
    if m < 3 || signed_area2(&pts2).abs() <= eps || self_intersects2(&pts2, eps) {
        return Ok(Vec::new()); // geometrically unbuildable input — empty, not an error
    }

    // The clip needs the base ring CCW (+1) against CW cutters (−1).
    if signed_area2(&pts2) < 0.0 {
        pts2.reverse();
        let old = dist2.clone();
        // Reversal turns edge k into traversed-backward edge (m-2-k) mod m, so
        // the per-edge distances are remapped the same way.
        for (k, slot) in dist2.iter_mut().enumerate() {
            *slot = old[(2 * m - 2 - k) % m];
        }
    }

    if dist2.iter().all(|d| *d == 0.0) {
        return Ok(vec![finalize_region(&pts2, &[], base_y)]); // no-op inset
    }

    let mut polys: Vec<Vec<Pt2>> = Vec::with_capacity(1 + m);
    polys.push(pts2.clone());
    for i in 0..m {
        if dist2[i] > 0.0 {
            polys.push(forbidden_stadium(pts2[i], pts2[(i + 1) % m], dist2[i]));
        }
    }

    Ok(resolve_polygons_positive(&polys, eps)
        .into_iter()
        .filter(|region| signed_area2(&region.outer).abs() > eps)
        .map(|region| finalize_region(&region.outer, &region.holes, base_y))
        .collect())
}

/// One polyline in an offset group: its centreline, stroke width, and closed flag.
pub struct OffsetPolyline {
    pub centreline: Vec<Vector3>,
    pub width: f64,
    pub closed: bool,
}

/// Merge a GROUP of separate polylines (a crossing T / X / L overlap) into one
/// clean stroked region: every polyline's per-segment MITERED bands are collected
/// and unioned by nonzero winding, so the overlapping strokes at the crossing merge
/// into a single region (no internal edges) with mitered/bevelled corners — the
/// same engine and corner treatment a single stroke uses. Pure 2D, no CSG. Empty
/// vec = nothing built.
pub fn offset_polyline_group_regions(
    polylines: &[OffsetPolyline],
    miter_limit: f64,
    eps: f64,
) -> Vec<OffsetRegion> {
    let mut bands: Vec<Vec<Pt2>> = Vec::new();
    let mut base_y = 0.0;
    let mut max_half = 0.0_f64;
    let mut have_base = false;
    for polyline in polylines {
        if polyline.width <= 0.0 || !polyline.width.is_finite() || polyline.centreline.len() < 2 {
            continue;
        }
        let half = polyline.width / 2.0;
        max_half = max_half.max(half);
        let simplified = simplify_polyline(&polyline.centreline, half, polyline.closed);
        if !have_base {
            base_y = simplified.first().map(|v| v.y).unwrap_or(0.0);
            have_base = true;
        }
        let raw: Vec<Pt2> = simplified.iter().map(xz).collect();
        let (pts, is_closed) = resolve_closed(&raw, polyline.closed, eps);
        bands.extend(mitered_offset_bands(
            &pts,
            is_closed,
            half,
            f64::INFINITY,
            eps,
        ));
    }
    if bands.is_empty() {
        return Vec::new();
    }
    union_polygons_nonzero(&bands, eps)
        .into_iter()
        .map(|region| {
            let outer = bevel_sharp_convex_pts(&region.outer, max_half, miter_limit, eps);
            finalize_region(&outer, &region.holes, base_y)
        })
        .collect()
}

// ── wasm boundary ────────────────────────────────────────────────────────────

/// One stroked region serialized for the JS boundary: CW outer ring + CCW holes.
#[derive(serde::Serialize)]
struct OffsetRegionJson {
    outer: Vec<Vector3>,
    holes: Vec<Vec<Vector3>>,
}

/// Result payload for a polyline offset: a list of regions (each an outer ring +
/// inner-void holes). Normally one region; a self-crossing closed centreline (e.g.
/// a figure-8) yields one per simple sub-loop. The deterministic analytic offset —
/// no extrude/solid. Used to build the plan polygons + extruded base solids in JS.
#[wasm_bindgen]
pub struct OGOffsetRegionsResult {
    regions_serialized: String,
}

impl OGOffsetRegionsResult {
    fn from_regions(regions: Vec<OffsetRegion>) -> Result<Self, String> {
        let json: Vec<OffsetRegionJson> = regions
            .into_iter()
            .map(|(outer, holes)| OffsetRegionJson { outer, holes })
            .collect();
        let regions_serialized = serde_json::to_string(&json)
            .map_err(|error| format!("Failed to serialize offset regions: {}", error))?;
        Ok(Self { regions_serialized })
    }
}

#[wasm_bindgen]
impl OGOffsetRegionsResult {
    #[wasm_bindgen(getter, js_name = regionsSerialized)]
    pub fn regions_serialized(&self) -> String {
        self.regions_serialized.clone()
    }
}

/// Wasm entry point: offset a centreline by `width` into stroked regions via the
/// deterministic analytic offset. `centreline_flat` is `[x0,y0,z0, …]`. Returns
/// CW-outer / CCW-hole regions, canonicalised; empty (does NOT throw) when degenerate.
#[wasm_bindgen(js_name = offsetPolylineRegions)]
pub fn offset_polyline_regions_wasm(
    centreline_flat: Vec<f64>,
    width: f64,
    closed: bool,
    miter_limit: f64,
) -> Result<OGOffsetRegionsResult, JsValue> {
    if centreline_flat.len() % 3 != 0 {
        return Err(JsValue::from_str(
            "Centreline must be a flat [x,y,z,…] array whose length is a multiple of 3",
        ));
    }
    let centreline: Vec<Vector3> = centreline_flat
        .chunks_exact(3)
        .map(|c| Vector3::new(c[0], c[1], c[2]))
        .collect();

    let limit = if miter_limit.is_finite() && miter_limit > 0.0 {
        miter_limit
    } else {
        DEFAULT_MITER_LIMIT
    };

    let regions = offset_polyline_regions(&centreline, width, closed, limit, DEFAULT_EPS);
    OGOffsetRegionsResult::from_regions(regions).map_err(|error| JsValue::from_str(&error))
}

/// One polyline in an offset group, as received over the JS boundary.
#[derive(serde::Deserialize)]
struct OffsetPolylineJson {
    centreline: Vec<f64>,
    width: f64,
    closed: bool,
}

/// Wasm entry point: merge a GROUP of separate polylines (a crossing T / X / L
/// overlap) into one clean stroked region by nonzero-winding union of their mitered
/// bands. `polylines_json` is `[{centreline:[x,y,z,…], width, closed}, …]`. Returns
/// CW-outer / CCW-hole regions (one or more); empty when nothing is buildable.
#[wasm_bindgen(js_name = offsetPolylineGroupRegions)]
pub fn offset_polyline_group_regions_wasm(
    polylines_json: String,
) -> Result<OGOffsetRegionsResult, JsValue> {
    let parsed: Vec<OffsetPolylineJson> =
        serde_json::from_str(&polylines_json).map_err(|error| {
            JsValue::from_str(&format!("Invalid offset group JSON payload: {}", error))
        })?;
    let polylines: Vec<OffsetPolyline> = parsed
        .into_iter()
        .filter(|polyline| polyline.centreline.len() % 3 == 0)
        .map(|polyline| OffsetPolyline {
            centreline: polyline
                .centreline
                .chunks_exact(3)
                .map(|c| Vector3::new(c[0], c[1], c[2]))
                .collect(),
            width: polyline.width,
            closed: polyline.closed,
        })
        .collect();

    let regions = offset_polyline_group_regions(&polylines, DEFAULT_MITER_LIMIT, DEFAULT_EPS);
    OGOffsetRegionsResult::from_regions(regions).map_err(|error| JsValue::from_str(&error))
}

/// Wasm entry point: inset a CLOSED ring inward with one distance per edge
/// (edge i = ring[i] → ring[i+1]). `ring_flat` is `[x0,y0,z0, …]`. Returns
/// CW-outer / CCW-hole regions, canonicalised; possibly several when a deep
/// inset splits the ring, and empty (does NOT throw) when the inset collapses
/// or the input ring is degenerate. THROWS on malformed arguments: a distance
/// count that does not match the edge count, or a negative / non-finite
/// distance. Corners where distances differ are circumscribed clearance arcs
/// (conservative), so no output point is ever closer to edge i than
/// `distances[i]` — there is no miter-limit knob because there are no miters.
#[wasm_bindgen(js_name = offsetRingVariable)]
pub fn offset_ring_variable_wasm(
    ring_flat: Vec<f64>,
    distances: Vec<f64>,
) -> Result<OGOffsetRegionsResult, JsValue> {
    if ring_flat.len() % 3 != 0 {
        return Err(JsValue::from_str(
            "Ring must be a flat [x,y,z,…] array whose length is a multiple of 3",
        ));
    }
    let ring: Vec<Vector3> = ring_flat
        .chunks_exact(3)
        .map(|c| Vector3::new(c[0], c[1], c[2]))
        .collect();

    let regions = offset_ring_variable(&ring, &distances, DEFAULT_EPS)
        .map_err(|error| JsValue::from_str(&error))?;
    OGOffsetRegionsResult::from_regions(regions).map_err(|error| JsValue::from_str(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, z: f64) -> Vector3 {
        Vector3::new(x, 0.0, z)
    }

    /// A many-pointed star centreline (alternating outer/inner radius) — the
    /// stress case with both sharp convex points and reflex notches.
    fn star_centreline(points: usize, outer: f64, inner: f64) -> Vec<Vector3> {
        let total = points * 2;
        let mut ring: Vec<Vector3> = Vec::with_capacity(total + 1);
        for i in 0..total {
            let radius = if i % 2 == 0 { outer } else { inner };
            let angle = (i as f64 / total as f64) * std::f64::consts::TAU;
            ring.push(pt(angle.cos() * radius, angle.sin() * radius));
        }
        ring.push(ring[0]); // close
        ring
    }

    /// Unwrap a result expected to be a single region.
    fn single(regions: Vec<OffsetRegion>) -> OffsetRegion {
        assert_eq!(regions.len(), 1, "expected exactly one region");
        regions.into_iter().next().unwrap()
    }

    /// A straight stroke offsets the centreline by exactly half-width.
    #[test]
    fn straight_offsets_by_half_width() {
        let centreline = vec![pt(0.0, 0.0), pt(2.0, 0.0)];
        let (outer, holes) = single(offset_polyline_regions(
            &centreline,
            0.3,
            false,
            DEFAULT_MITER_LIMIT,
            DEFAULT_EPS,
        ));
        assert_eq!(outer.len(), 4, "a straight stroke is a 4-point rectangle");
        assert!(holes.is_empty(), "an open stroke has no holes");
        // Centreline runs along +x at z=0; offsets should be at z = ±0.15.
        for v in &outer {
            assert!((v.z.abs() - 0.15).abs() < 1e-9, "offset z = {}", v.z);
        }
    }

    /// An X-crossing of two separate strokes merges into one cross-shaped region
    /// (no holes), with the crossing centre filled — not two overlapping rects.
    #[test]
    fn x_group_merges_into_one_region() {
        let polylines = vec![
            OffsetPolyline {
                centreline: vec![pt(-3.0, 0.0), pt(3.0, 0.0)],
                width: 0.4,
                closed: false,
            },
            OffsetPolyline {
                centreline: vec![pt(0.0, -3.0), pt(0.0, 3.0)],
                width: 0.4,
                closed: false,
            },
        ];
        let regions = offset_polyline_group_regions(&polylines, DEFAULT_MITER_LIMIT, DEFAULT_EPS);
        assert_eq!(regions.len(), 1, "an X-crossing merges into one region");
        let (outer, holes) = &regions[0];
        assert!(holes.is_empty(), "a simple cross has no interior void");
        let ring: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(
            !self_intersects2(&ring, DEFAULT_EPS),
            "merged outline is simple"
        );
        assert!(
            point_in_ring2(Pt2::new(0.0, 0.0), &ring),
            "crossing centre is filled"
        );
        // A plus sign has 12 corners; certainly more than a single rectangle's 4.
        assert!(
            outer.len() >= 8,
            "cross outline has the crossing's corners ({} pts)",
            outer.len()
        );
    }

    /// An open L stroke is one simple ring with no holes.
    #[test]
    fn open_l_is_a_simple_ring() {
        let centreline = vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(4.0, 2.0)];
        let (outer, holes) = single(offset_polyline_regions(
            &centreline,
            0.3,
            false,
            DEFAULT_MITER_LIMIT,
            DEFAULT_EPS,
        ));
        assert!(holes.is_empty(), "open chain has no holes");
        let ring: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(ring.len() >= 4, "ring has at least 4 points");
        assert!(
            !self_intersects2(&ring, DEFAULT_EPS),
            "outer ring must be simple"
        );
        assert!(
            signed_area2(&ring).abs() > DEFAULT_EPS,
            "outer ring must have area"
        );
    }

    /// A closed square loop produces an outer ring plus exactly one interior-void hole.
    #[test]
    fn closed_square_has_outer_and_one_hole() {
        let centreline = vec![
            pt(0.0, 0.0),
            pt(4.0, 0.0),
            pt(4.0, 4.0),
            pt(0.0, 4.0),
            pt(0.0, 0.0),
        ];
        let (outer, holes) = single(offset_polyline_regions(
            &centreline,
            0.4,
            true,
            DEFAULT_MITER_LIMIT,
            DEFAULT_EPS,
        ));
        assert!(outer.len() >= 4, "outer ring has at least 4 points");
        assert_eq!(
            holes.len(),
            1,
            "closed loop has exactly one interior-void hole"
        );
        let outer2: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(
            !self_intersects2(&outer2, DEFAULT_EPS),
            "outer ring must be simple"
        );
    }

    /// The offset is deterministic: the same input yields identical regions every call.
    #[test]
    fn offset_is_deterministic() {
        let centreline = star_centreline(24, 6.0, 3.3);
        let same = |a: &[Vector3], b: &[Vector3]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(p, q)| {
                    (p.x - q.x).abs() < 1e-12
                        && (p.y - q.y).abs() < 1e-12
                        && (p.z - q.z).abs() < 1e-12
                })
        };
        let baseline =
            offset_polyline_regions(&centreline, 0.3, true, DEFAULT_MITER_LIMIT, DEFAULT_EPS);
        assert!(!baseline.is_empty(), "star offset builds");
        for _ in 0..8 {
            let again =
                offset_polyline_regions(&centreline, 0.3, true, DEFAULT_MITER_LIMIT, DEFAULT_EPS);
            assert_eq!(baseline.len(), again.len(), "region count must be stable");
            for ((o1, h1), (o2, h2)) in baseline.iter().zip(&again) {
                assert!(same(o1, o2), "outer ring must be identical across calls");
                assert_eq!(h1.len(), h2.len(), "hole count must be stable");
                for (a, b) in h1.iter().zip(h2) {
                    assert!(same(a, b), "hole rings must be identical across calls");
                }
            }
        }
    }

    /// A stroke as wide as its loop (1×1 loop, 1.0-wide stroke): the inner offset
    /// collapses, so the region is a SOLID block (no interior void), not a failure.
    #[test]
    fn tight_closed_loop_builds_a_solid_region() {
        let centreline = vec![
            pt(0.0, 0.0),
            pt(1.0, 0.0),
            pt(1.0, 1.0),
            pt(0.0, 1.0),
            pt(0.0, 0.0),
        ];
        let (outer, holes) = single(offset_polyline_regions(
            &centreline,
            1.0,
            true,
            DEFAULT_MITER_LIMIT,
            DEFAULT_EPS,
        ));
        assert!(
            holes.is_empty(),
            "the interior void is consumed → no hole (solid)"
        );
        let outer2: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(
            outer.len() >= 4 && !self_intersects2(&outer2, DEFAULT_EPS),
            "outer ring is a simple block"
        );
        assert!(
            signed_area2(&outer2).abs() > DEFAULT_EPS,
            "outer ring has area"
        );
    }

    /// A self-crossing centreline (figure-8): the per-segment bands UNION by nonzero
    /// winding so the overlapping strokes at the crossing MERGE — the crossing centre is
    /// filled (not duplicated/internal-edged), and each lobe interior is a void.
    #[test]
    fn self_crossing_centreline_merges_at_the_crossing() {
        let bowtie = vec![
            pt(0.0, 0.0),
            pt(3.0, 3.0),
            pt(3.0, 0.0),
            pt(0.0, 3.0),
            pt(0.0, 0.0),
        ];
        let regions = offset_polyline_regions(&bowtie, 0.3, true, DEFAULT_MITER_LIMIT, DEFAULT_EPS);
        assert!(!regions.is_empty(), "figure-8 builds");
        for (outer, _holes) in &regions {
            let ring: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
            assert!(
                ring.len() >= 3 && !self_intersects2(&ring, DEFAULT_EPS),
                "each region outer ring is simple (crossing merged, no crossing edges)"
            );
        }
        // `p` is covered iff inside some region's outer ring and not in any of its holes.
        let filled = |p: Pt2| -> bool {
            regions.iter().any(|(outer, holes)| {
                let o: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
                if !point_in_ring2(p, &o) {
                    return false;
                }
                !holes.iter().any(|hole| {
                    let h: Vec<Pt2> = hole.iter().map(|v| Pt2::new(v.x, v.z)).collect();
                    point_in_ring2(p, &h)
                })
            })
        };
        // The crossing centre (1.5,1.5) is MERGED — the two diagonal bands overlap there.
        assert!(
            filled(Pt2::new(1.5, 1.5)),
            "crossing centre is filled (merged)"
        );
        // A point deep inside a lobe triangle is a void (the area the stroke encloses).
        assert!(
            !filled(Pt2::new(0.6, 1.5)),
            "lobe interior is an empty void"
        );
    }

    /// An acute open corner bevels (no runaway miter spike): the region stays a
    /// simple ring bounded near the centreline rather than spiking far out.
    #[test]
    fn acute_open_corner_bevels_and_stays_bounded() {
        let centreline = vec![pt(0.0, 0.0), pt(5.0, 0.0), pt(2.0, 1.0)];
        let (outer, holes) = single(offset_polyline_regions(
            &centreline,
            0.5,
            false,
            DEFAULT_MITER_LIMIT,
            DEFAULT_EPS,
        ));
        assert!(holes.is_empty(), "open stroke has no holes");
        let ring: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(
            !self_intersects2(&ring, DEFAULT_EPS),
            "outer ring is simple"
        );
        // A full miter at this acute apex would spike well past the centreline; the
        // bevel keeps every vertex within ~width of the centreline bbox (x≤5).
        assert!(
            outer.iter().all(|v| v.x < 6.0),
            "no runaway miter spike (bevelled)"
        );
    }

    /// A doubling-back OPEN path (reflex hairpin) resolves by nonzero winding to a
    /// single simple outline — no spur, no self-crossing.
    #[test]
    fn self_overlapping_open_path_resolves_simple() {
        let centreline = vec![pt(0.0, 0.0), pt(5.0, 0.0), pt(1.0, 0.4)];
        let (outer, _holes) = single(offset_polyline_regions(
            &centreline,
            0.3,
            false,
            DEFAULT_MITER_LIMIT,
            DEFAULT_EPS,
        ));
        let ring: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(
            !self_intersects2(&ring, DEFAULT_EPS),
            "resolved outer ring is simple"
        );
        assert!(
            signed_area2(&ring).abs() > DEFAULT_EPS,
            "outer ring has area"
        );
    }

    #[test]
    fn simplify_drops_subthreshold_jog_but_keeps_endpoint() {
        let centreline = vec![pt(0.0, 0.0), pt(1.0, 0.0), pt(1.0, 0.001), pt(3.0, 0.0)];
        let simplified = simplify_polyline(&centreline, 0.1, false);
        // The 0.001 jog collapses; endpoints are preserved.
        assert!(simplified.len() < centreline.len());
        assert!((simplified[0].x - 0.0).abs() < 1e-9);
        let last = simplified.last().unwrap();
        assert!((last.x - 3.0).abs() < 1e-9, "true endpoint preserved");
    }

    // ── offset_ring_variable ─────────────────────────────────────────────────

    fn variable(ring: &[Vector3], distances: &[f64]) -> Result<Vec<OffsetRegion>, String> {
        offset_ring_variable(ring, distances, DEFAULT_EPS)
    }

    fn ring2(region: &OffsetRegion) -> Vec<Pt2> {
        region.0.iter().map(|v| Pt2::new(v.x, v.z)).collect()
    }

    /// Min distance from `p` to the segment a→b.
    fn dist_to_segment(p: Pt2, a: Pt2, b: Pt2) -> f64 {
        let abx = b.x - a.x;
        let abz = b.z - a.z;
        let len2 = abx * abx + abz * abz;
        let t = if len2 > 0.0 {
            (((p.x - a.x) * abx + (p.z - a.z) * abz) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (p.x - (a.x + t * abx)).hypot(p.z - (a.z + t * abz))
    }

    fn has_vertex(ring: &[Vector3], x: f64, z: f64) -> bool {
        ring.iter()
            .any(|v| (v.x - x).abs() < 1e-9 && (v.z - z).abs() < 1e-9)
    }

    /// A uniform variable inset of a square equals the plain shrunken square.
    #[test]
    fn variable_inset_uniform_matches_square() {
        let ring = vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 20.0), pt(0.0, 20.0)];
        let regions = variable(&ring, &[2.0; 4]).expect("valid arguments");
        let (outer, holes) = single(regions);
        assert!(holes.is_empty(), "an inward inset cannot create holes");
        assert_eq!(outer.len(), 4, "square inset stays a quad");
        for (x, z) in [(2.0, 2.0), (18.0, 2.0), (18.0, 18.0), (2.0, 18.0)] {
            assert!(has_vertex(&outer, x, z), "missing corner ({}, {})", x, z);
        }
    }

    /// Front/side/rear setbacks on a rectangle land each edge at its own distance.
    #[test]
    fn variable_inset_front_side_rear_setbacks() {
        // 20 (x) × 30 (z); edge 0 = front (z=0, setback 6), edges 1/3 = sides
        // (x=20 and x=0, setback 1.5), edge 2 = rear (z=30, setback 3).
        let ring = vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 30.0), pt(0.0, 30.0)];
        let regions = variable(&ring, &[6.0, 1.5, 3.0, 1.5]).expect("valid arguments");
        let (outer, _) = single(regions);
        assert_eq!(outer.len(), 4);
        for (x, z) in [(1.5, 6.0), (18.5, 6.0), (18.5, 27.0), (1.5, 27.0)] {
            assert!(has_vertex(&outer, x, z), "missing corner ({}, {})", x, z);
        }
        let area = signed_area2(&ring2(&(outer.clone(), Vec::new()))).abs();
        assert!((area - 17.0 * 21.0).abs() < 1e-9, "area = {}", area);
    }

    /// A zero-distance edge stays exactly in place (lot-line construction).
    #[test]
    fn variable_inset_zero_distance_edge_stays() {
        let ring = vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 30.0), pt(0.0, 30.0)];
        let regions = variable(&ring, &[0.0, 1.5, 3.0, 1.5]).expect("valid arguments");
        let (outer, _) = single(regions);
        // The front edge (z = 0) is untouched: its inset corners sit on it.
        assert!(has_vertex(&outer, 1.5, 0.0));
        assert!(has_vertex(&outer, 18.5, 0.0));
    }

    /// All-zero distances are a legitimate no-op inset returning the input ring.
    #[test]
    fn variable_inset_all_zero_returns_input_ring() {
        let ring = vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 30.0), pt(0.0, 30.0)];
        let (outer, holes) = single(variable(&ring, &[0.0; 4]).expect("valid arguments"));
        assert!(holes.is_empty());
        assert_eq!(outer.len(), 4);
        for (x, z) in [(0.0, 0.0), (20.0, 0.0), (20.0, 30.0), (0.0, 30.0)] {
            assert!(has_vertex(&outer, x, z));
        }
    }

    #[test]
    fn variable_inset_distance_count_mismatch_errors() {
        let ring = vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 30.0), pt(0.0, 30.0)];
        let Err(error) = variable(&ring, &[6.0, 1.5, 3.0]) else {
            panic!("count mismatch must throw");
        };
        assert!(
            error.contains("4 edges"),
            "message names the edge count: {}",
            error
        );
        assert!(
            error.contains("3 distances"),
            "message names the given count: {}",
            error
        );
    }

    #[test]
    fn variable_inset_negative_distance_errors() {
        let ring = vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 30.0), pt(0.0, 30.0)];
        let Err(error) = variable(&ring, &[6.0, -1.5, 3.0, 1.5]) else {
            panic!("negative distance must throw");
        };
        assert!(
            error.contains("distances[1]"),
            "message names the index: {}",
            error
        );
    }

    /// A ring given with an explicit closing duplicate takes the same N distances
    /// and produces the identical canonical result.
    #[test]
    fn variable_inset_accepts_closing_duplicate() {
        let open = vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 30.0), pt(0.0, 30.0)];
        let mut closed = open.clone();
        closed.push(open[0]);
        let distances = [6.0, 1.5, 3.0, 1.5];
        let a = single(variable(&open, &distances).expect("open form"));
        let b = single(variable(&closed, &distances).expect("closed form"));
        assert_eq!(a.0.len(), b.0.len());
        for (p, q) in a.0.iter().zip(&b.0) {
            assert!((p.x - q.x).abs() < 1e-12 && (p.z - q.z).abs() < 1e-12);
        }
    }

    /// A CW ring with reversal-remapped distances matches the CCW result.
    #[test]
    fn variable_inset_cw_input_matches_ccw() {
        let ccw = vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 30.0), pt(0.0, 30.0)];
        let ccw_d = [6.0, 1.5, 3.0, 1.5];
        // Reversed point order: edge k of the CW ring is edge (n-2-k) mod n of
        // the CCW ring traversed backward.
        let cw: Vec<Vector3> = ccw.iter().rev().copied().collect();
        let n = ccw.len();
        let cw_d: Vec<f64> = (0..n).map(|k| ccw_d[(2 * n - 2 - k) % n]).collect();
        let a = single(variable(&ccw, &ccw_d).expect("ccw"));
        let b = single(variable(&cw, &cw_d).expect("cw"));
        assert_eq!(a.0.len(), b.0.len());
        for (p, q) in a.0.iter().zip(&b.0) {
            assert!((p.x - q.x).abs() < 1e-12 && (p.z - q.z).abs() < 1e-12);
        }
    }

    /// Splitting one frontage into two collinear edges with different distances
    /// transitions between the two setback lines through the deeper edge's
    /// clearance arc around the split point — NOT a perpendicular step, which
    /// would claim points within the deep distance of the deep segment's
    /// endpoint.
    #[test]
    fn variable_inset_collinear_split_edge_steps() {
        let ring = vec![
            pt(0.0, 0.0),
            pt(10.0, 0.0), // split point on the z=0 frontage
            pt(20.0, 0.0),
            pt(20.0, 30.0),
            pt(0.0, 30.0),
        ];
        let regions = variable(&ring, &[2.0, 5.0, 1.5, 3.0, 1.5]).expect("valid arguments");
        let (outer, _) = single(regions);
        let r2 = ring2(&(outer.clone(), Vec::new()));
        assert!(
            !self_intersects2(&r2, DEFAULT_EPS),
            "stepped envelope is simple"
        );
        // Both setback lines exist…
        assert!(
            outer.iter().any(|v| (v.z - 2.0).abs() < 1e-9 && v.x < 5.5),
            "shallow frontage line at z=2 (left of the deep edge's clearance arc)"
        );
        assert!(
            outer
                .iter()
                .any(|v| (v.z - 5.0).abs() < 1e-9 && v.x > 10.0 - 1e-9),
            "deep frontage line at z=5"
        );
        // …joined through the arc anchored at (10, 5) above the split point.
        assert!(has_vertex(&outer, 10.0, 5.0), "arc anchor above the split");
        // The would-be step corner (10, 2) is within 5 m of the deep frontage
        // segment, so it must NOT be claimed.
        let deep_a = Pt2::new(10.0, 0.0);
        let deep_b = Pt2::new(20.0, 0.0);
        for v in &outer {
            assert!(
                dist_to_segment(Pt2::new(v.x, v.z), deep_a, deep_b) >= 5.0 - 1.0e-6,
                "vertex ({}, {}) violates the deep frontage clearance",
                v.x,
                v.z
            );
        }
    }

    /// Every output vertex keeps at least its edge's distance to that edge —
    /// the compliance property a setback envelope exists to guarantee. Cases
    /// include a sharp reflex notch (where a miter/bevel join would spike or
    /// under-clear) and a deep notch whose far flank constrains points across
    /// exterior space.
    #[test]
    fn variable_inset_clearance_respected() {
        let cases: Vec<(Vec<Vector3>, Vec<f64>)> = vec![
            (
                vec![pt(0.0, 0.0), pt(20.0, 0.0), pt(20.0, 30.0), pt(0.0, 30.0)],
                vec![6.0, 1.5, 3.0, 1.5],
            ),
            (
                // Reflex notch in the top edge (sharp spike pointing into the lot).
                vec![
                    pt(0.0, 0.0),
                    pt(40.0, 0.0),
                    pt(40.0, 30.0),
                    pt(22.0, 30.0),
                    pt(20.0, 12.0), // notch tip — sharp reflex on both flanks
                    pt(18.0, 30.0),
                    pt(0.0, 30.0),
                ],
                vec![3.0, 2.0, 1.0, 2.5, 2.5, 1.0, 4.0],
            ),
        ];
        for (ring, distances) in cases {
            let raw: Vec<Pt2> = ring.iter().map(|v| Pt2::new(v.x, v.z)).collect();
            let regions = offset_ring_variable(&ring, &distances, DEFAULT_EPS).expect("valid");
            assert!(!regions.is_empty(), "inset builds");
            for region in &regions {
                for v in &region.0 {
                    let p = Pt2::new(v.x, v.z);
                    for i in 0..raw.len() {
                        let clearance = dist_to_segment(p, raw[i], raw[(i + 1) % raw.len()]);
                        assert!(
                            clearance >= distances[i] - 1.0e-6,
                            "vertex ({}, {}) is {} from edge {} (required {})",
                            p.x,
                            p.z,
                            clearance,
                            i,
                            distances[i]
                        );
                    }
                }
            }
        }
    }

    /// Clearance applies to edge SEGMENTS across exterior space: a narrow
    /// notch's far lot line constrains the envelope on the near side, exactly
    /// like the service's per-edge compliance checker measures it.
    #[test]
    fn variable_inset_respects_distant_edges_across_a_notch() {
        // A 2-wide exterior slot (x 10..12) cut into the top of a 30×20 lot.
        let ring = vec![
            pt(0.0, 0.0),
            pt(30.0, 0.0),
            pt(30.0, 20.0),
            pt(12.0, 20.0),
            pt(12.0, 8.0),
            pt(10.0, 8.0), // slot bottom
            pt(10.0, 20.0),
            pt(0.0, 20.0),
        ];
        // Edge 5 = slot's left wall (10,8)→(10,20) faces the right lobe across
        // the 2-wide slot with a 5 m clearance: it must carve into x ∈ (12, 15).
        let distances = vec![1.0, 1.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0];
        let regions = variable(&ring, &distances).expect("valid arguments");
        assert!(!regions.is_empty());
        let wall_a = Pt2::new(10.0, 8.0);
        let wall_b = Pt2::new(10.0, 20.0);
        for region in &regions {
            for v in &region.0 {
                assert!(
                    dist_to_segment(Pt2::new(v.x, v.z), wall_a, wall_b) >= 5.0 - 1.0e-6,
                    "vertex ({}, {}) is inside the slot wall's clearance",
                    v.x,
                    v.z
                );
            }
        }
        // The point (13, 14) is only 3 m from the slot's left wall — across
        // exterior space — and must be excluded from every region.
        let probe = Pt2::new(13.0, 14.0);
        for region in &regions {
            let r2 = ring2(region);
            assert!(
                !point_in_ring2(probe, &r2),
                "(13, 14) violates the slot wall clearance but was claimed"
            );
        }
    }

    /// Distances that consume the whole ring collapse to an empty result.
    #[test]
    fn variable_inset_collapse_returns_empty() {
        let ring = vec![pt(0.0, 0.0), pt(4.0, 0.0), pt(4.0, 4.0), pt(0.0, 4.0)];
        let regions = variable(&ring, &[3.0; 4]).expect("valid arguments");
        assert!(
            regions.is_empty(),
            "fully consumed inset is empty, not an error"
        );
    }

    /// A waisted (dumbbell) parcel whose waist is narrower than the side insets
    /// splits into TWO disjoint envelope regions — the case the old
    /// largest-hole stroke trick could not represent.
    #[test]
    fn variable_inset_dumbbell_splits_into_two_regions() {
        let dumbbell = vec![
            pt(0.0, 0.0),
            pt(20.0, 0.0),
            pt(20.0, 14.0), // waist starts: narrow corridor z 14..26 at x 8..12
            pt(12.0, 14.0),
            pt(12.0, 26.0),
            pt(20.0, 26.0),
            pt(20.0, 40.0),
            pt(0.0, 40.0),
            pt(0.0, 26.0),
            pt(8.0, 26.0),
            pt(8.0, 14.0),
            pt(0.0, 14.0),
        ];
        // The corridor is 4 wide (x 8..12); a uniform 3.0 inset closes it but
        // leaves both 20×14 lobes buildable.
        let regions = variable(&dumbbell, &[3.0; 12]).expect("valid arguments");
        assert_eq!(regions.len(), 2, "pinched waist splits the envelope in two");
        let original: Vec<Pt2> = dumbbell.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        for region in &regions {
            let r2 = ring2(region);
            assert!(
                r2.len() >= 3 && !self_intersects2(&r2, DEFAULT_EPS),
                "each lobe is simple"
            );
            assert!(signed_area2(&r2).abs() > 1.0, "each lobe has real area");
            assert!(
                ring_inside2(&r2, &original, DEFAULT_EPS),
                "lobes stay inside the parcel"
            );
        }
    }

    /// Degenerate rings (too few points, zero area, self-intersecting) are
    /// empty results, not errors.
    #[test]
    fn variable_inset_degenerate_ring_is_empty() {
        let two = vec![pt(0.0, 0.0), pt(10.0, 0.0)];
        assert!(variable(&two, &[1.0, 1.0])
            .expect("valid arguments")
            .is_empty());
        let collinear = vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(20.0, 0.0)];
        assert!(variable(&collinear, &[1.0; 3])
            .expect("valid arguments")
            .is_empty());
        let bowtie = vec![pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 0.0), pt(0.0, 10.0)];
        assert!(variable(&bowtie, &[1.0; 4])
            .expect("valid arguments")
            .is_empty());
    }

    /// The variable inset is deterministic across repeated calls.
    #[test]
    fn variable_inset_deterministic() {
        let ring = vec![
            pt(0.0, 0.0),
            pt(40.0, 0.0),
            pt(40.0, 30.0),
            pt(22.0, 30.0),
            pt(20.0, 12.0),
            pt(18.0, 30.0),
            pt(0.0, 30.0),
        ];
        let distances = [3.0, 2.0, 1.0, 2.5, 2.5, 1.0, 4.0];
        let same = |a: &[Vector3], b: &[Vector3]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(p, q)| {
                    (p.x - q.x).abs() < 1e-12
                        && (p.y - q.y).abs() < 1e-12
                        && (p.z - q.z).abs() < 1e-12
                })
        };
        let baseline = variable(&ring, &distances).expect("valid");
        assert!(!baseline.is_empty());
        for _ in 0..8 {
            let again = variable(&ring, &distances).expect("valid");
            assert_eq!(baseline.len(), again.len());
            for ((o1, h1), (o2, h2)) in baseline.iter().zip(&again) {
                assert!(same(o1, o2), "outer ring identical across calls");
                assert_eq!(h1.len(), h2.len());
            }
        }
    }
}
