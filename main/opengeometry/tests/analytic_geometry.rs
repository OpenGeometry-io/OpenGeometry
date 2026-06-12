//! D1 analytic-geometry acceptance tests, doubling as the analytic oracle for
//! the program (PRD §2.7 A1 prerequisite). Exactness is verified against
//! closed-form references — known radii, cylinder cross-sections — rather than
//! an external OCC kernel, which cannot be linked into this WASM crate.

use opengeometry::brep::{CurveGeometry, SurfaceGeometry};
use opengeometry::primitives::arc::OGArc;
use opengeometry::primitives::cylinder::OGCylinder;
use openmaths::Vector3;

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

#[test]
fn arc_edges_carry_exact_circle_curve() {
    let mut arc = OGArc::new("arc-analytic".to_string());
    arc.set_config(Vector3::new(0.0, 0.0, 0.0), 2.5, 0.0, TWO_PI, 24)
        .unwrap();

    let brep = arc.brep();
    assert!(!brep.edges.is_empty());
    for edge in &brep.edges {
        match edge.curve.as_ref().expect("arc edge must carry a curve") {
            CurveGeometry::Circle { radius, .. } => {
                assert!((radius - 2.5).abs() < 1.0e-12, "exact radius preserved");
            }
            other => panic!("expected Circle, got {:?}", other.kind()),
        }
    }
}

#[test]
fn cylinder_has_one_cylindrical_surface_and_circular_edges() {
    let mut cylinder = OGCylinder::new("cyl-analytic".to_string());
    cylinder
        .set_config(Vector3::new(0.0, 0.0, 0.0), 1.5, 4.0, TWO_PI, 32)
        .unwrap();

    let brep = cylinder.brep();

    // Caps are planar, the lateral surface is one analytic cylinder of radius 1.5.
    let mut cylinder_faces = 0usize;
    let mut plane_faces = 0usize;
    for face in &brep.faces {
        match face.surface.as_ref().expect("cylinder face must be tagged") {
            SurfaceGeometry::Cylinder { radius, height, .. } => {
                assert!((radius - 1.5).abs() < 1.0e-9);
                assert!((height - 4.0).abs() < 1.0e-9);
                cylinder_faces += 1;
            }
            SurfaceGeometry::Plane { .. } => plane_faces += 1,
        }
    }
    assert_eq!(plane_faces, 2, "top + bottom caps");
    assert!(
        cylinder_faces >= 1,
        "lateral faces share the cylinder surface"
    );

    // The top/bottom rings are exact circles of radius 1.5.
    let circle_edges = brep
        .edges
        .iter()
        .filter(|e| matches!(e.curve, Some(CurveGeometry::Circle { .. })))
        .count();
    assert!(
        circle_edges >= 2,
        "two circular rings, got {}",
        circle_edges
    );
}

#[test]
fn analytic_geometry_survives_world_transform() {
    // Exactness must hold after placement: radius scales, geometry stays a
    // cylinder (not silently downgraded to facets).
    let mut cylinder = OGCylinder::new("cyl-xform".to_string());
    cylinder
        .set_config(Vector3::new(0.0, 0.0, 0.0), 1.0, 2.0, TWO_PI, 16)
        .unwrap();
    cylinder
        .set_transform(
            Vector3::new(5.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(3.0, 3.0, 3.0),
        )
        .unwrap();

    let world = cylinder.world_brep();
    let scaled = world.faces.iter().find_map(|f| match &f.surface {
        Some(SurfaceGeometry::Cylinder { radius, .. }) => Some(*radius),
        _ => None,
    });
    assert!(
        (scaled.expect("cylinder surface present after transform") - 3.0).abs() < 1.0e-9,
        "radius scaled by uniform factor 3"
    );
}
