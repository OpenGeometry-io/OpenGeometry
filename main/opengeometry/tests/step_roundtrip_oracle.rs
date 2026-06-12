//! STEP exact-geometry round-trip oracle (debt items D1/D9, NFR-4).
//!
//! NFR-4 requires a unit cylinder exported to STEP to round-trip through a
//! reference reader as a single cylindrical face, not a facet fan. A real
//! OpenCASCADE kernel cannot be linked into this WASM crate, so this oracle
//! independently *re-parses* the exported Part-21 text — acting as the reference
//! reader — and checks that the analytic cylinder/circle radii it recovers match
//! the source geometry exactly. (An optional pythonocc/OCP cross-check is in
//! `tests/occ_oracle.rs`, skipped when OCC is absent.)

use opengeometry::export::step::{export_brep_to_step_text, StepExportConfig};
use opengeometry::primitives::cylinder::OGCylinder;
use openmaths::Vector3;

/// Pulls the trailing real argument out of each `KEYWORD(...,radius)` entity —
/// the reference reader re-deriving the analytic radius from the file.
fn parse_trailing_reals(text: &str, keyword: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let needle = format!("{}(", keyword);
    let mut rest = text;
    while let Some(pos) = rest.find(&needle) {
        let after = &rest[pos + needle.len()..];
        if let Some(end) = after.find(')') {
            let args = &after[..end];
            if let Some(last) = args.rsplit(',').next() {
                if let Ok(v) = last.trim().parse::<f64>() {
                    out.push(v);
                }
            }
            rest = &after[end..];
        } else {
            break;
        }
    }
    out
}

#[test]
fn exported_cylinder_round_trips_as_one_analytic_cylinder() {
    let radius = 1.5;
    let height = 4.0;
    let mut cyl = OGCylinder::new("oracle-cyl".into());
    cyl.set_config(
        Vector3::new(0.0, 0.0, 0.0),
        radius,
        height,
        2.0 * std::f64::consts::PI,
        48,
    )
    .unwrap();
    let brep = cyl.world_brep();

    let (text, _) =
        export_brep_to_step_text(&brep, &StepExportConfig::default()).expect("step export");

    // Reference reader: exactly one cylindrical surface, recovered at the exact
    // source radius — not a fan of planar facets.
    let cyl_radii = parse_trailing_reals(&text, "CYLINDRICAL_SURFACE");
    assert_eq!(
        cyl_radii.len(),
        1,
        "exactly one cylindrical surface in file"
    );
    assert!(
        (cyl_radii[0] - radius).abs() < 1.0e-9,
        "round-tripped cylinder radius {} != source {}",
        cyl_radii[0],
        radius
    );

    // The circular rings round-trip at the same exact radius.
    let circle_radii = parse_trailing_reals(&text, "CIRCLE");
    assert!(!circle_radii.is_empty(), "circular edges present");
    for r in &circle_radii {
        assert!(
            (r - radius).abs() < 1.0e-9,
            "circle radius {} != source {}",
            r,
            radius
        );
    }

    // No planar facet fan stands in for the curved wall: the cylinder is one
    // advanced face, so advanced faces are few (cylinder + 2 caps).
    assert!(
        text.matches("ADVANCED_FACE").count() <= 4,
        "cylinder wall must not be a facet fan"
    );
}

#[test]
fn exported_cylinder_scales_radius_with_export_scale() {
    let mut cyl = OGCylinder::new("oracle-scale".into());
    cyl.set_config(
        Vector3::new(0.0, 0.0, 0.0),
        1.0,
        2.0,
        2.0 * std::f64::consts::PI,
        24,
    )
    .unwrap();
    let brep = cyl.world_brep();

    let config = StepExportConfig {
        scale: 10.0,
        ..StepExportConfig::default()
    };
    let (text, _) = export_brep_to_step_text(&brep, &config).expect("scaled export");

    let cyl_radii = parse_trailing_reals(&text, "CYLINDRICAL_SURFACE");
    assert_eq!(cyl_radii.len(), 1);
    assert!(
        (cyl_radii[0] - 10.0).abs() < 1.0e-9,
        "radius should scale by export scale, got {}",
        cyl_radii[0]
    );
}
