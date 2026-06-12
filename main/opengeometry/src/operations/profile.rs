//! Profile robustness (debt item D7).
//!
//! Generated and imported profiles routinely contain artifacts — wrong winding,
//! coincident/duplicate points, degenerate edges, self-intersections. The
//! kernel previously assumed clean CCW-outer/CW-hole input and only computed
//! winding in the hardcoded XZ plane; ear-clipping then silently produced
//! garbage faces on bad input with no diagnostic. This module validates a loop
//! before face creation: it cleans duplicates, computes winding on the loop's
//! actual plane (any orientation), auto-corrects to CCW-outer, and detects
//! self-intersection and degeneracy — reporting what it found rather than
//! emitting a malformed face.

use openmaths::Vector3;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::geometry::poly2d::{self_intersects2, signed_area2, Pt2, DEFAULT_EPS};
use crate::operations::triangulate::compute_polygon_normal;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProfileReport {
    /// The loop has at least 3 non-degenerate vertices and does not self-intersect.
    pub is_valid: bool,
    /// Duplicate / coincident vertices removed during cleanup.
    pub removed_duplicates: usize,
    /// Winding was reversed to make the outer loop CCW (in its own plane).
    pub winding_corrected: bool,
    /// Two non-adjacent edges cross — a face built from this loop is invalid.
    pub self_intersecting: bool,
    /// Fewer than 3 vertices remain, or the loop encloses ~zero area.
    pub degenerate: bool,
    /// Human-readable diagnostics for each issue found.
    pub diagnostics: Vec<String>,
}

/// Analyzes (and conceptually repairs) a profile loop. Returns the report plus
/// the cleaned, correctly-wound points ready for face creation. The returned
/// points are empty when the loop is too degenerate to use.
pub fn analyze_profile(points: &[Vector3]) -> (ProfileReport, Vec<Vector3>) {
    let mut report = ProfileReport::default();

    // 1. Remove consecutive duplicates and a closing duplicate.
    let mut cleaned: Vec<Vector3> = Vec::with_capacity(points.len());
    for p in points {
        let dup = cleaned
            .last()
            .map_or(false, |last: &Vector3| coincident(*last, *p));
        if dup {
            report.removed_duplicates += 1;
        } else {
            cleaned.push(*p);
        }
    }
    if cleaned.len() > 1 && coincident(cleaned[0], *cleaned.last().unwrap()) {
        cleaned.pop();
        report.removed_duplicates += 1;
    }
    if report.removed_duplicates > 0 {
        report.diagnostics.push(format!(
            "removed {} duplicate vertices",
            report.removed_duplicates
        ));
    }

    if cleaned.len() < 3 {
        report.degenerate = true;
        report
            .diagnostics
            .push("fewer than 3 unique vertices".into());
        return (report, Vec::new());
    }

    // 2. Project onto the loop's dominant plane to reason in 2D.
    let normal = compute_polygon_normal(&cleaned).unwrap_or(Vector3::new(0.0, 1.0, 0.0));
    let ring = project_to_plane(&cleaned, normal);
    let area = signed_area2(&ring);

    // 3. Self-intersection. Checked first: a bowtie has ~zero signed area (its
    // sub-triangles cancel) yet is self-intersecting, not merely degenerate.
    if self_intersects2(&ring, DEFAULT_EPS) {
        report.self_intersecting = true;
        report
            .diagnostics
            .push("loop self-intersects (non-adjacent edges cross)".into());
    }

    // 4. Degeneracy: ~zero enclosed area and not self-intersecting (collinear).
    if !report.self_intersecting && area.abs() <= DEFAULT_EPS {
        report.degenerate = true;
        report
            .diagnostics
            .push("loop encloses ~zero area (collinear/degenerate)".into());
        return (report, Vec::new());
    }

    // 5. Winding: make a sound outer loop CCW in its projection plane.
    if !report.self_intersecting && area < 0.0 {
        cleaned.reverse();
        report.winding_corrected = true;
        report
            .diagnostics
            .push("winding reversed to CCW-outer".into());
    }

    report.is_valid = !report.self_intersecting && !report.degenerate;
    let out = if report.is_valid { cleaned } else { Vec::new() };
    (report, out)
}

/// Wasm entry point: analyze a flat `[x0,y0,z0,…]` loop, returning the report as
/// JSON (the corrected points are not returned here; use the kernel API).
#[wasm_bindgen(js_name = analyzeProfile)]
pub fn analyze_profile_wasm(flat_points: Vec<f64>) -> Result<String, JsValue> {
    let points: Vec<Vector3> = flat_points
        .chunks_exact(3)
        .map(|c| Vector3::new(c[0], c[1], c[2]))
        .collect();
    let (report, _) = analyze_profile(&points);
    serde_json::to_string(&report)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize profile report: {}", e)))
}

fn coincident(a: Vector3, b: Vector3) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz) <= DEFAULT_EPS * DEFAULT_EPS
}

/// Projects 3D points onto a 2D ring in the plane whose normal is `normal`,
/// dropping the dominant normal axis (so winding/area are computed in-plane).
fn project_to_plane(points: &[Vector3], normal: Vector3) -> Vec<Pt2> {
    let (ax, az) = if normal.y.abs() >= normal.x.abs() && normal.y.abs() >= normal.z.abs() {
        (0, 2) // dominant Y → project to XZ
    } else if normal.x.abs() >= normal.z.abs() {
        (1, 2) // dominant X → project to YZ
    } else {
        (0, 1) // dominant Z → project to XY
    };
    let comp = |v: &Vector3, i: usize| match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    };
    points
        .iter()
        .map(|v| Pt2::new(comp(v, ax), comp(v, az)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_ccw_square_is_valid_unchanged() {
        let square = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];
        let (report, pts) = analyze_profile(&square);
        assert!(report.is_valid);
        assert_eq!(pts.len(), 4);
    }

    #[test]
    fn clockwise_loop_is_corrected() {
        // Clockwise in XZ → negative area → reversed.
        let cw = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(1.0, 0.0, 0.0),
        ];
        let (report, _) = analyze_profile(&cw);
        assert!(report.winding_corrected);
        assert!(report.is_valid);
    }

    #[test]
    fn duplicate_points_are_removed() {
        let dup = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];
        let (report, pts) = analyze_profile(&dup);
        assert_eq!(report.removed_duplicates, 1);
        assert_eq!(pts.len(), 4);
    }

    #[test]
    fn bowtie_is_flagged_self_intersecting() {
        // Classic self-intersecting "bowtie" quad.
        let bowtie = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 1.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ];
        let (report, _) = analyze_profile(&bowtie);
        assert!(report.self_intersecting);
        assert!(!report.is_valid);
    }

    #[test]
    fn collinear_points_are_degenerate() {
        let line = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
        ];
        let (report, pts) = analyze_profile(&line);
        assert!(report.degenerate);
        assert!(pts.is_empty());
    }

    #[test]
    fn winding_works_on_tilted_plane() {
        // A loop on the XY plane (normal +Z) must still get a correct winding
        // verdict — the old XZ-only signed area would have read ~zero here.
        let square_xy = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ];
        let (report, pts) = analyze_profile(&square_xy);
        assert!(!report.degenerate, "XY loop must not read as zero-area");
        assert_eq!(pts.len(), 4);
    }
}
