//! Fixed-width polyline offset: centreline + width -> stroked region(s), plus a
//! group offset that merges several overlapping strokes into one. Built on the
//! generic `geometry` 2D toolkit. Domain-neutral (walls, roads, pipe routes,
//! hatching, font strokes, etc.).

use openmaths::Vector3;
use wasm_bindgen::prelude::*;
use crate::geometry::poly2d::*;
use crate::geometry::offset2d::*;
use crate::geometry::boolean2d::*;

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
        ring.iter().map(|p| Vector3::new(p.x, base_y, p.z)).collect()
    };
    let outer = canonical_ring(&normalize_winding(&to_v3(outer), Winding::Cw));
    let holes = hole_rings
        .iter()
        .map(|hole| canonical_ring(&normalize_winding(&to_v3(hole), Winding::Ccw)))
        .collect();
    (outer, holes)
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
        bands.extend(mitered_offset_bands(&pts, is_closed, half, f64::INFINITY, eps));
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
    let parsed: Vec<OffsetPolylineJson> = serde_json::from_str(&polylines_json)
        .map_err(|error| JsValue::from_str(&format!("Invalid offset group JSON payload: {}", error)))?;
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
        let (outer, holes) =
            single(offset_polyline_regions(&centreline, 0.3, false, DEFAULT_MITER_LIMIT, DEFAULT_EPS));
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
        assert!(!self_intersects2(&ring, DEFAULT_EPS), "merged outline is simple");
        assert!(point_in_ring2(Pt2::new(0.0, 0.0), &ring), "crossing centre is filled");
        // A plus sign has 12 corners; certainly more than a single rectangle's 4.
        assert!(outer.len() >= 8, "cross outline has the crossing's corners ({} pts)", outer.len());
    }

    /// An open L stroke is one simple ring with no holes.
    #[test]
    fn open_l_is_a_simple_ring() {
        let centreline = vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(4.0, 2.0)];
        let (outer, holes) =
            single(offset_polyline_regions(&centreline, 0.3, false, DEFAULT_MITER_LIMIT, DEFAULT_EPS));
        assert!(holes.is_empty(), "open chain has no holes");
        let ring: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(ring.len() >= 4, "ring has at least 4 points");
        assert!(!self_intersects2(&ring, DEFAULT_EPS), "outer ring must be simple");
        assert!(signed_area2(&ring).abs() > DEFAULT_EPS, "outer ring must have area");
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
        let (outer, holes) =
            single(offset_polyline_regions(&centreline, 0.4, true, DEFAULT_MITER_LIMIT, DEFAULT_EPS));
        assert!(outer.len() >= 4, "outer ring has at least 4 points");
        assert_eq!(holes.len(), 1, "closed loop has exactly one interior-void hole");
        let outer2: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(!self_intersects2(&outer2, DEFAULT_EPS), "outer ring must be simple");
    }

    /// The offset is deterministic: the same input yields identical regions every call.
    #[test]
    fn offset_is_deterministic() {
        let centreline = star_centreline(24, 6.0, 3.3);
        let same = |a: &[Vector3], b: &[Vector3]| {
            a.len() == b.len()
                && a.iter().zip(b).all(|(p, q)| {
                    (p.x - q.x).abs() < 1e-12 && (p.y - q.y).abs() < 1e-12 && (p.z - q.z).abs() < 1e-12
                })
        };
        let baseline = offset_polyline_regions(&centreline, 0.3, true, DEFAULT_MITER_LIMIT, DEFAULT_EPS);
        assert!(!baseline.is_empty(), "star offset builds");
        for _ in 0..8 {
            let again = offset_polyline_regions(&centreline, 0.3, true, DEFAULT_MITER_LIMIT, DEFAULT_EPS);
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
        let (outer, holes) =
            single(offset_polyline_regions(&centreline, 1.0, true, DEFAULT_MITER_LIMIT, DEFAULT_EPS));
        assert!(holes.is_empty(), "the interior void is consumed → no hole (solid)");
        let outer2: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(outer.len() >= 4 && !self_intersects2(&outer2, DEFAULT_EPS), "outer ring is a simple block");
        assert!(signed_area2(&outer2).abs() > DEFAULT_EPS, "outer ring has area");
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
        assert!(filled(Pt2::new(1.5, 1.5)), "crossing centre is filled (merged)");
        // A point deep inside a lobe triangle is a void (the area the stroke encloses).
        assert!(!filled(Pt2::new(0.6, 1.5)), "lobe interior is an empty void");
    }

    /// An acute open corner bevels (no runaway miter spike): the region stays a
    /// simple ring bounded near the centreline rather than spiking far out.
    #[test]
    fn acute_open_corner_bevels_and_stays_bounded() {
        let centreline = vec![pt(0.0, 0.0), pt(5.0, 0.0), pt(2.0, 1.0)];
        let (outer, holes) =
            single(offset_polyline_regions(&centreline, 0.5, false, DEFAULT_MITER_LIMIT, DEFAULT_EPS));
        assert!(holes.is_empty(), "open stroke has no holes");
        let ring: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(!self_intersects2(&ring, DEFAULT_EPS), "outer ring is simple");
        // A full miter at this acute apex would spike well past the centreline; the
        // bevel keeps every vertex within ~width of the centreline bbox (x≤5).
        assert!(outer.iter().all(|v| v.x < 6.0), "no runaway miter spike (bevelled)");
    }

    /// A doubling-back OPEN path (reflex hairpin) resolves by nonzero winding to a
    /// single simple outline — no spur, no self-crossing.
    #[test]
    fn self_overlapping_open_path_resolves_simple() {
        let centreline = vec![pt(0.0, 0.0), pt(5.0, 0.0), pt(1.0, 0.4)];
        let (outer, _holes) =
            single(offset_polyline_regions(&centreline, 0.3, false, DEFAULT_MITER_LIMIT, DEFAULT_EPS));
        let ring: Vec<Pt2> = outer.iter().map(|v| Pt2::new(v.x, v.z)).collect();
        assert!(!self_intersects2(&ring, DEFAULT_EPS), "resolved outer ring is simple");
        assert!(signed_area2(&ring).abs() > DEFAULT_EPS, "outer ring has area");
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
}
