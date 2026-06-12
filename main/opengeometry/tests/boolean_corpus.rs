//! Boolean regression corpus with a pass-rate gate (debt item D4, NFR-2 / US-4.3).
//!
//! Robustness is *measured, not assumed*: this builds a corpus of multi-body
//! sketch-extrude-style boolean cases (overlapping cuboids/cylinders/spheres at
//! varied offsets) and asserts the success rate clears the 90% gate without any
//! per-case manual tolerance tuning. A "success" is a boolean that returns a
//! structured result whose B-rep passes geometric validity (D5) — never silent
//! garbage. The realized rate is printed so regressions are visible in CI logs.

use opengeometry::booleans::types::BooleanOptions;
use opengeometry::booleans::{boolean_intersection, boolean_subtraction, boolean_union};
use opengeometry::brep::validity::check_validity;
use opengeometry::brep::Brep;
use opengeometry::primitives::cuboid::OGCuboid;
use opengeometry::primitives::cylinder::OGCylinder;
use opengeometry::primitives::sphere::OGSphere;
use openmaths::Vector3;

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

fn cuboid(center: Vector3, w: f64, h: f64, d: f64) -> Brep {
    let mut c = OGCuboid::new("corpus-cuboid".into());
    c.set_config(center, w, h, d).unwrap();
    c.world_brep()
}

fn cylinder(center: Vector3, r: f64, h: f64) -> Brep {
    let mut c = OGCylinder::new("corpus-cyl".into());
    c.set_config(center, r, h, TWO_PI, 24).unwrap();
    c.world_brep()
}

fn sphere(center: Vector3, r: f64) -> Brep {
    let mut s = OGSphere::new("corpus-sphere".into());
    s.set_config(center, r, 18, 12).unwrap();
    s.world_brep()
}

enum Op {
    Union,
    Subtract,
    Intersect,
}

struct Case {
    name: &'static str,
    lhs: Brep,
    rhs: Brep,
    op: Op,
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::new();

    // Overlapping cuboids at a range of offsets (the bread-and-butter CSG case).
    for (i, offset) in [0.5_f64, 1.0, 1.5, 1.9].iter().enumerate() {
        let lhs = cuboid(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0);
        let rhs = cuboid(Vector3::new(*offset, *offset, 0.0), 2.0, 2.0, 2.0);
        let op = match i % 3 {
            0 => Op::Union,
            1 => Op::Subtract,
            _ => Op::Intersect,
        };
        cases.push(Case {
            name: "cuboid-cuboid",
            lhs,
            rhs,
            op,
        });
    }

    // Sphere cut from / unioned with a cuboid.
    for (i, r) in [0.8_f64, 1.0, 1.2].iter().enumerate() {
        let lhs = cuboid(Vector3::new(0.0, 0.0, 0.0), 3.0, 3.0, 3.0);
        let rhs = sphere(Vector3::new(0.5, 0.5, 0.5), *r);
        let op = if i % 2 == 0 { Op::Subtract } else { Op::Union };
        cases.push(Case {
            name: "cuboid-sphere",
            lhs,
            rhs,
            op,
        });
    }

    // Cylinder through a block (drilled hole) at several positions.
    for offset in [0.0_f64, 0.4, 0.8] {
        let lhs = cuboid(Vector3::new(0.0, 0.0, 0.0), 3.0, 2.0, 3.0);
        let rhs = cylinder(Vector3::new(offset, 0.0, 0.0), 0.6, 4.0);
        cases.push(Case {
            name: "block-drill",
            lhs,
            rhs,
            op: Op::Subtract,
        });
    }

    // Intersections of overlapping spheres/cuboids.
    cases.push(Case {
        name: "sphere-sphere-intersect",
        lhs: sphere(Vector3::new(0.0, 0.0, 0.0), 1.5),
        rhs: sphere(Vector3::new(1.0, 0.0, 0.0), 1.5),
        op: Op::Intersect,
    });

    cases
}

#[test]
fn boolean_corpus_clears_ninety_percent_gate() {
    let cases = corpus();
    let total = cases.len();
    let mut successes = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for case in cases {
        let options = BooleanOptions::default();
        let result = match case.op {
            Op::Union => boolean_union(&case.lhs, &case.rhs, options),
            Op::Subtract => boolean_subtraction(&case.lhs, &case.rhs, options),
            Op::Intersect => boolean_intersection(&case.lhs, &case.rhs, options),
        };

        match result {
            Ok(output) => {
                let report = check_validity(&output.brep);
                // A valid solid OR a deliberately-empty result both count as a
                // non-garbage outcome; an invalid non-empty B-rep does not.
                if output.report.empty || report.closed_shell {
                    successes += 1;
                } else {
                    failures.push(format!("{}: result failed validity", case.name));
                }
            }
            // A structured error is a *reported* failure (not garbage), but it
            // still counts against the success rate for this corpus.
            Err(err) => failures.push(format!("{}: {}", case.name, err)),
        }
    }

    let rate = successes as f64 / total as f64;
    println!(
        "boolean corpus: {}/{} succeeded ({:.1}%)",
        successes,
        total,
        rate * 100.0
    );
    for f in &failures {
        println!("  fail: {}", f);
    }

    assert!(
        rate >= 0.90,
        "boolean success rate {:.1}% is below the 90% gate (NFR-2)",
        rate * 100.0
    );
}

#[test]
fn boolean_results_carry_analytic_planar_surfaces() {
    // D4/D1 analytic-aware: faces emerging from a boolean are tagged with an
    // analytic Plane so the result also exports as analytic geometry (D9), not
    // bare facets.
    use opengeometry::brep::SurfaceGeometry;
    let lhs = cuboid(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0);
    let rhs = cuboid(Vector3::new(1.0, 0.0, 0.0), 2.0, 2.0, 2.0);
    let out = boolean_union(&lhs, &rhs, BooleanOptions::default()).expect("union");

    assert!(!out.brep.faces.is_empty());
    let tagged = out
        .brep
        .faces
        .iter()
        .filter(|f| matches!(f.surface, Some(SurfaceGeometry::Plane { .. })))
        .count();
    assert_eq!(
        tagged,
        out.brep.faces.len(),
        "every boolean-output face should carry an analytic plane"
    );
}

#[test]
fn drilled_hole_wall_survives_as_analytic_cylinder() {
    // D4/D1 re-detection: subtract a cylinder from a block; the hole wall faces
    // lie on the cutter cylinder and must be retagged Cylinder (not flattened).
    use opengeometry::brep::SurfaceGeometry;
    let block = cuboid(Vector3::new(0.0, 0.0, 0.0), 3.0, 2.0, 3.0);
    let drill = cylinder(Vector3::new(0.0, 0.0, 0.0), 0.6, 4.0);
    let out = boolean_subtraction(&block, &drill, BooleanOptions::default()).expect("drill");

    let cyl_faces = out
        .brep
        .faces
        .iter()
        .filter(|f| matches!(f.surface, Some(SurfaceGeometry::Cylinder { .. })))
        .count();
    assert!(
        cyl_faces >= 1,
        "drilled hole wall should be analytic cylinder, got {} cylinder faces",
        cyl_faces
    );
}

#[test]
fn booleans_are_deterministic_across_runs() {
    // NFR-1: identical inputs produce identical topology across runs.
    let lhs = cuboid(Vector3::new(0.0, 0.0, 0.0), 2.0, 2.0, 2.0);
    let rhs = cuboid(Vector3::new(1.0, 0.0, 0.0), 2.0, 2.0, 2.0);

    let a = boolean_union(&lhs, &rhs, BooleanOptions::default()).expect("union a");
    let b = boolean_union(&lhs, &rhs, BooleanOptions::default()).expect("union b");

    assert_eq!(a.brep.vertices.len(), b.brep.vertices.len());
    assert_eq!(a.brep.faces.len(), b.brep.faces.len());
    assert_eq!(a.brep.edges.len(), b.brep.edges.len());
}
